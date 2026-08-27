use std::sync::Arc;
use tethera_common::protocol::error::{EntityKind, WireError};
use tethera_common::protocol::transfer::PutSpec;
use tethera_common::structs::asset::AssetScope;
use tethera_common::structs::ids::{AssetId, ConversationId};
use tethera_common::structs::primitives::Sha256;
use tethera_common::structs::terminal::Size;
use tethera_server_lib::backend::TerminalBackend;
use tethera_server_lib::config::ApplicationConfig;
use tethera_server_lib::protocol::live::{LiveAssets, LiveConversations, LiveTerminals};
use tethera_server_lib::protocol::ports::AssetPort;
use tethera_server_lib::terminal::PaneRegistry;
use tethera_server_lib::transcript::AssetIndex;

/// A machine with its own upload directory and no files in it.
struct Machine {
    _data: tempfile::TempDir,
    _home: tempfile::TempDir,
    assets: LiveAssets,
}

impl Machine {
    fn empty() -> Self {
        let data = tempfile::tempdir().expect("a data directory");
        let home = tempfile::tempdir().expect("a home");
        let config = ApplicationConfig::with_data_dir(data.path().to_path_buf());

        let panes = PaneRegistry::new_shared();
        let backend = Arc::new(TerminalBackend::herdr(
            "a-binary-nothing-resolves".to_string(),
            Size { cols: 80, rows: 24 },
        ));

        let index = AssetIndex::new_shared();
        let conversations = Arc::new(LiveConversations::at(
            LiveTerminals::new_shared(backend, panes),
            home.path(),
            index.clone(),
            config.data_dir.join("uploads"),
        ));

        Self {
            assets: LiveAssets::new(&config, conversations, index),
            _data: data,
            _home: home,
        }
    }

    /// The same machine again, with a fresh index over the same directory.
    ///
    /// What a restart is: the files survive and everything remembered about them
    /// does not.
    fn restarted(&self) -> LiveAssets {
        let config = ApplicationConfig::with_data_dir(self._data.path().to_path_buf());
        let panes = PaneRegistry::new_shared();
        let backend = Arc::new(TerminalBackend::herdr(
            "a-binary-nothing-resolves".to_string(),
            Size { cols: 80, rows: 24 },
        ));

        let index = AssetIndex::new_shared();
        let conversations = Arc::new(LiveConversations::at(
            LiveTerminals::new_shared(backend, panes),
            self._home.path(),
            index.clone(),
            config.data_dir.join("uploads"),
        ));

        LiveAssets::new(&config, conversations, index)
    }

    /// A fetch answers a reader, not a buffer, so a test that wants the bytes
    /// reads them the way the transfer layer does.
    fn drain(mut body: std::fs::File) -> Vec<u8> {
        use std::io::Read as _;

        let mut bytes = Vec::new();
        body.read_to_end(&mut bytes).expect("the body reads");

        bytes
    }

    fn digest_of(body: &[u8]) -> Sha256 {
        use sha2::Digest as _;

        Sha256(format!("{:x}", sha2::Sha256::digest(body)))
    }

    fn spec(name: &str, body: &[u8]) -> PutSpec {
        PutSpec {
            name: name.to_string(),
            len: body.len() as u64,
            sha256: Self::digest_of(body),
            offset: 0,
        }
    }
}

// The id is a one-way hash of a path, so the only ids this machine can serve are
// the ones it has read a record about. Anything else is a reference to a file it
// has never heard of, and serving it would mean guessing.
#[tokio::test]
async fn an_id_this_machine_has_never_read_about_is_not_found() {
    let machine = Machine::empty();

    assert!(matches!(
        machine.assets.fetch(&AssetId::mint("nothing"), 0).await,
        Err(WireError::NotFound {
            kind: EntityKind::Asset
        })
    ));
}

#[tokio::test]
async fn a_conversation_with_no_records_lists_no_files() {
    let machine = Machine::empty();
    let listed = machine
        .assets
        .list(
            &AssetScope::Conversation(ConversationId::mint("never-written")),
            None,
            10,
        )
        .await;

    // A conversation nobody has records for is not found; one with records and
    // no files lists nothing. Either way there is no card.
    assert!(listed.map(|page| page.items.is_empty()).unwrap_or(true));
}

// Refused before the bytes rather than after them. Over a relayed link the
// difference is a refusal against a wasted minute, which is why the bound is
// also stated in `Describe.limits`.
#[tokio::test]
async fn an_upload_past_the_stated_bound_is_refused_before_it_starts() {
    let machine = Machine::empty();
    let mut spec = Machine::spec("enormous.bin", b"x");
    spec.len = LiveAssets::MAX_UPLOAD + 1;

    assert!(matches!(
        machine.assets.put_ready(&spec).await,
        Err(WireError::TooLarge { .. })
    ));
}

// Nothing of a previous attempt is on disk, so the client is told to start at
// the beginning. Only this machine knows how much arrived, which is why the
// answer is authoritative over the offset the client proposed.
#[tokio::test]
async fn a_fresh_upload_is_told_to_start_at_the_beginning() {
    let machine = Machine::empty();
    let ready = machine
        .assets
        .put_ready(&Machine::spec("notes.txt", b"hello"))
        .await
        .expect("ready");

    assert_eq!(ready.offset, 0);
}

// The whole round trip: an upload arrives, is verified, gets an id, and that id
// fetches back exactly what was sent.
#[tokio::test]
async fn an_upload_can_be_fetched_back_by_the_id_it_was_given() {
    let machine = Machine::empty();
    let body = b"the quick brown fox";
    let spec = Machine::spec("fox.txt", body);

    machine.assets.put_ready(&spec).await.expect("ready");

    let stored = machine
        .assets
        .put_finish(&spec, body)
        .await
        .expect("the upload was kept");

    let (head, body) = machine
        .assets
        .fetch(&stored.asset, 0)
        .await
        .expect("the upload fetches back");

    let fetched = Machine::drain(body);

    assert_eq!(fetched, b"the quick brown fox");
    assert_eq!(head.len, fetched.len() as u64);
    assert_eq!(head.offset, 0);
    assert_eq!(
        head.sha256, spec.sha256,
        "the head has to carry the digest of the whole asset; a client deletes its \
         download when this does not match"
    );
}

// A truncated upload is a shorter file that opens perfectly well, so the digest
// is the only moment it can be caught. Issuing an id for it would put a file in
// front of an agent that is not the file the person sent.
#[tokio::test]
async fn an_upload_that_does_not_match_its_digest_is_thrown_away() {
    let machine = Machine::empty();
    let spec = Machine::spec("fox.txt", b"the quick brown fox");

    machine.assets.put_ready(&spec).await.expect("ready");

    let refused = machine.assets.put_finish(&spec, b"the quick brown").await;

    assert!(matches!(refused, Err(WireError::Backend { .. })));

    // And nothing was kept, so the next attempt starts clean rather than
    // appending onto the corrupt remains of this one.
    assert_eq!(
        machine
            .assets
            .put_ready(&spec)
            .await
            .expect("ready again")
            .offset,
        0
    );
}

// A resumed upload sends only the remainder and seeks to the offset this machine
// answers. An optimistic offset here silently truncates the file, and the digest
// check is what would catch it — after the whole transfer.
#[tokio::test]
async fn a_resumed_upload_is_told_what_actually_arrived() {
    let machine = Machine::empty();
    let body = b"the quick brown fox";
    let spec = Machine::spec("fox.txt", body);

    machine.assets.put_ready(&spec).await.expect("ready");

    // An attempt that stopped partway: the bytes reached disk, the digest did
    // not match, and it was refused.
    let _ = machine.assets.put_finish(&spec, &body[..8]).await;

    // The refusal discarded the partial file, so the next attempt is told to
    // start over rather than to append onto something that was thrown away.
    assert_eq!(
        machine
            .assets
            .put_ready(&spec)
            .await
            .expect("ready")
            .offset,
        0
    );

    let stored = machine
        .assets
        .put_finish(&spec, body)
        .await
        .expect("the whole file");

    let (_, reader) = machine.assets.fetch(&stored.asset, 0).await.expect("fetch");

    assert_eq!(Machine::drain(reader), body);
}

// Resuming a download sends the remainder, and the head still reports the whole
// asset — its length and its digest — because that is what the client checks the
// finished file against.
#[tokio::test]
async fn a_fetch_from_an_offset_sends_the_remainder_and_describes_the_whole() {
    let machine = Machine::empty();
    let body = b"the quick brown fox";
    let spec = Machine::spec("fox.txt", body);

    machine.assets.put_ready(&spec).await.expect("ready");
    let stored = machine.assets.put_finish(&spec, body).await.expect("kept");

    let (head, reader) = machine
        .assets
        .fetch(&stored.asset, 10)
        .await
        .expect("the remainder");

    assert_eq!(Machine::drain(reader), &body[10..]);
    assert_eq!(head.offset, 10);
    assert_eq!(head.len, body.len() as u64, "the whole asset, not the range");
    assert_eq!(head.sha256, spec.sha256, "the whole asset's digest");
}

// The index lives in memory and the uploads live on disk. A file an agent handed
// over is registered again whenever its transcript is read, so it heals itself —
// but an upload is registered exactly once, at `put_finish`. Without recovery,
// every restart silently orphans every file a person had sent: the bytes are
// still there and no id reaches them again.
#[tokio::test]
async fn an_upload_survives_the_machine_restarting() {
    let machine = Machine::empty();
    let body = b"still here tomorrow";
    let spec = Machine::spec("survivor.txt", body);

    machine.assets.put_ready(&spec).await.expect("ready");
    let stored = machine.assets.put_finish(&spec, body).await.expect("kept");

    // A fresh index and a fresh port over the same directory, which is what a
    // restart is.
    let restarted = machine.restarted();

    let (_, reader) = restarted
        .fetch(&stored.asset, 0)
        .await
        .expect("the upload is still reachable after a restart");

    assert_eq!(Machine::drain(reader), body);
}

// An upload that never finished has no id, and giving it one would offer a file
// that is a prefix of itself.
#[tokio::test]
async fn a_half_finished_upload_is_not_recovered_as_a_file() {
    let machine = Machine::empty();
    let body = b"the quick brown fox";
    let spec = Machine::spec("fox.txt", body);

    machine.assets.put_ready(&spec).await.expect("ready");

    // Enough of an attempt to leave a `.part` behind, and not enough to pass its
    // digest.
    let _ = machine.assets.put_finish(&spec, &body[..6]).await;

    // Whatever is on disk, nothing partial became fetchable.
    let restarted = machine.restarted();

    for id in [AssetId::mint("anything"), AssetId::mint(&spec.sha256.0)] {
        assert!(matches!(
            restarted.fetch(&id, 0).await,
            Err(WireError::NotFound { .. })
        ));
    }
}

// Sending the same file twice is the ordinary case, not an edge one: a person
// sends one screenshot to two conversations, or retries after a failure. Both
// uploads share a digest, so both stage to the same partial file and store under
// the same name.
#[tokio::test]
async fn the_same_file_uploaded_twice_is_fetchable_both_times() {
    let machine = Machine::empty();
    let body = b"one screenshot, sent to two conversations";
    let spec = Machine::spec("screenshot.jpg", body);

    machine.assets.put_ready(&spec).await.expect("ready once");
    let first = machine.assets.put_finish(&spec, body).await.expect("kept once");

    machine.assets.put_ready(&spec).await.expect("ready twice");
    let again = machine
        .assets
        .put_finish(&spec, body)
        .await
        .expect("the second upload of the same file is kept too");

    assert_eq!(
        first.asset, again.asset,
        "the same bytes at the same path are the same asset"
    );

    let (_, reader) = machine
        .assets
        .fetch(&again.asset, 0)
        .await
        .expect("the id from the second upload resolves");

    assert_eq!(Machine::drain(reader), body);
}

// A client that already has everything asks past the end. Answering with an
// error would make it delete a file it had downloaded correctly.
#[tokio::test]
async fn a_fetch_past_the_end_is_an_empty_body_rather_than_a_failure() {
    let machine = Machine::empty();
    let body = b"short";
    let spec = Machine::spec("short.txt", body);

    machine.assets.put_ready(&spec).await.expect("ready");
    let stored = machine.assets.put_finish(&spec, body).await.expect("kept");

    let (head, reader) = machine
        .assets
        .fetch(&stored.asset, 9_000)
        .await
        .expect("an answer");

    assert!(Machine::drain(reader).is_empty());
    assert_eq!(head.offset, body.len() as u64);
    assert_eq!(head.len, body.len() as u64);
}
