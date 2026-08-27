//! The two bulk carriers, driven against a machine scripted by hand.
//!
//! Every test here is about a byte count, because that is the whole of what
//! these types decide. The body is raw, so nothing in the framing catches a
//! transfer that stopped early - which is exactly why a truncated file reads
//! perfectly well and reaches the disk as a silent corruption.

use std::time::Duration;
use tethera_client_core::transfer::{Digest, Fetch, Next, Partial, Put, Retry};
use tethera_common::protocol::stream::StreamOpen;
use tethera_common::protocol::transfer::{FetchHead, FetchSpec, PutReady, PutResult, PutSpec};
use tethera_common::structs::ids::AssetId;
use tethera_common::structs::primitives::Sha256;
use tethera_transport::frame::FrameCodec;
use tethera_transport::stream::testing::Loopback;
use tethera_transport::stream::FrameIo;

fn head(len: u64, offset: u64) -> FetchHead {
    FetchHead {
        len,
        mime: None,
        sha256: Sha256("00".to_string()),
        offset,
    }
}

/// A machine that answers one download with `head`, then `body`, then FIN.
async fn serve_fetch(connection: iroh::endpoint::Connection, answer: FetchHead, body: Vec<u8>) {
    let (mut send, mut recv) = connection.accept_bi().await.expect("accept");
    let codec = FrameCodec::default();

    let open: Option<StreamOpen> = FrameIo::read(&mut recv, &codec).await.expect("open");
    assert!(matches!(open, Some(StreamOpen::Fetch(_))));

    FrameIo::write(&mut send, &codec, &answer).await.expect("head");
    send.write_all(&body).await.expect("body");
    send.finish().ok();
}

#[tokio::test]
async fn a_download_that_stops_early_is_an_error_not_a_short_file() {
    let wire = Loopback::connect().await.expect("loopback");
    // Promises sixteen bytes and sends four, the way a machine that died
    // mid-read does.
    let machine = tokio::spawn(serve_fetch(wire.server.clone(), head(16, 0), vec![7u8; 4]));

    let (_, mut fetch) = Fetch::open(
        &wire.client,
        FetchSpec {
            asset: AssetId::mint("a"),
            offset: 0,
        },
    )
    .await
    .expect("open");

    let mut read = 0usize;
    let mut failed = false;

    loop {
        match fetch.next().await {
            Ok(Some(chunk)) => read += chunk.len(),
            Ok(None) => break,
            Err(_) => {
                failed = true;
                break;
            }
        }
    }

    machine.await.ok();

    assert!(
        failed,
        "a truncated download reported success after {read} of 16 bytes"
    );
    assert!(!fetch.complete());
}

#[tokio::test]
async fn a_resumed_download_counts_from_where_the_machine_starts() {
    let wire = Loopback::connect().await.expect("loopback");
    // A 16-byte asset resumed at 10, so only 6 bytes are actually coming. A
    // carrier that subtracted nothing would wait for 16 and call this complete
    // transfer a truncation.
    let machine = tokio::spawn(serve_fetch(wire.server.clone(), head(16, 10), vec![7u8; 6]));

    let (answer, mut fetch) = Fetch::open(
        &wire.client,
        FetchSpec {
            asset: AssetId::mint("a"),
            offset: 4,
        },
    )
    .await
    .expect("open");

    let mut read = 0usize;

    while let Some(chunk) = fetch.next().await.expect("chunk") {
        read += chunk.len();
    }

    machine.await.ok();

    // The machine's offset is believed over the one that was asked for.
    assert_eq!(answer.offset, 10);
    assert_eq!(read, 6);
    assert!(fetch.complete());
}

#[tokio::test]
async fn what_has_arrived_is_the_whole_length_less_what_remains() {
    let wire = Loopback::connect().await.expect("loopback");
    // A 16-byte asset resumed at 10, so six bytes are coming and ten are
    // already on this phone. Both halves count as arrived: a bar drawn from
    // what crossed the wire alone finishes at six sixteenths on a transfer
    // that is whole.
    let machine = tokio::spawn(serve_fetch(wire.server.clone(), head(16, 10), vec![7u8; 6]));

    let (answer, mut fetch) = Fetch::open(
        &wire.client,
        FetchSpec {
            asset: AssetId::mint("a"),
            offset: 10,
        },
    )
    .await
    .expect("open");

    assert_eq!(fetch.remaining(), 6);

    fetch.next().await.expect("chunk");

    machine.await.ok();

    assert_eq!(fetch.remaining(), 0);
    assert_eq!(answer.len - fetch.remaining(), 16);
}

#[tokio::test]
async fn a_download_that_loses_its_connection_says_what_lost_it() {
    let wire = Loopback::connect().await.expect("loopback");
    let server = wire.server.clone();
    let (opened, wait) = tokio::sync::oneshot::channel::<()>();

    // Promises sixteen bytes, sends four, then takes the whole connection with
    // it - the way a phone moved to another app takes its radio. The wait is
    // what keeps this a lost body rather than a lost head: closing before the
    // client has read the head tests the wrong call.
    let machine = tokio::spawn(async move {
        let (mut send, mut recv) = server.accept_bi().await.expect("accept");
        let codec = FrameCodec::default();

        let open: Option<StreamOpen> = FrameIo::read(&mut recv, &codec).await.expect("open");
        assert!(matches!(open, Some(StreamOpen::Fetch(_))));

        FrameIo::write(&mut send, &codec, &head(16, 0))
            .await
            .expect("head");
        send.write_all(&[7u8; 4]).await.expect("body");

        wait.await.ok();

        server.close(9u32.into(), b"the machine went away");
        server.closed().await;
    });

    let (_, mut fetch) = Fetch::open(
        &wire.client,
        FetchSpec {
            asset: AssetId::mint("a"),
            offset: 0,
        },
    )
    .await
    .expect("open");

    opened.send(()).ok();

    let failed = fetch
        .next()
        .await
        .expect_err("a lost connection is not a chunk");

    machine.await.ok();

    // `ReadError::ConnectionLost` Displays as those two words and nothing else,
    // so the reason the peer gave is reachable only through the source chain.
    // Without it every lost transfer reports the same undiagnosable sentence.
    let said = failed.to_string();

    assert!(
        said.contains("the machine went away"),
        "the message says nothing about what lost the connection: {said}"
    );
}

/// A machine that accepts one upload starting at `offset`.
async fn serve_put(connection: iroh::endpoint::Connection, offset: u64) {
    let (mut send, mut recv) = connection.accept_bi().await.expect("accept");
    let codec = FrameCodec::default();

    let open: Option<StreamOpen> = FrameIo::read(&mut recv, &codec).await.expect("open");
    assert!(matches!(open, Some(StreamOpen::Put(_))));

    FrameIo::write(&mut send, &codec, &PutReady { offset })
        .await
        .expect("ready");

    recv.read_to_end(64 * 1024).await.ok();

    FrameIo::write(
        &mut send,
        &codec,
        &PutResult {
            asset: AssetId::mint("stored"),
        },
    )
    .await
    .expect("result");

    send.finish().ok();
}

#[tokio::test]
async fn an_upload_refuses_to_send_more_than_it_declared() {
    let wire = Loopback::connect().await.expect("loopback");
    let machine = tokio::spawn(serve_put(wire.server.clone(), 0));

    let (_, mut put) = Put::open(
        &wire.client,
        PutSpec {
            name: "photo.png".to_string(),
            len: 8,
            sha256: Sha256("00".to_string()),
            offset: 0,
        },
    )
    .await
    .expect("open");

    put.write(&[1u8; 8]).await.expect("the declared body");

    // The machine allocated for eight bytes and accounted eight against a
    // quota. A ninth is this client breaking its own contract.
    let overrun = put.write(&[1u8; 1]).await;

    machine.abort();

    assert!(overrun.is_err(), "an upload sent past its declared length");
}

#[tokio::test]
async fn an_upload_that_stops_short_is_never_finished() {
    let wire = Loopback::connect().await.expect("loopback");
    let machine = tokio::spawn(serve_put(wire.server.clone(), 0));

    let (_, mut put) = Put::open(
        &wire.client,
        PutSpec {
            name: "photo.png".to_string(),
            len: 8,
            sha256: Sha256("00".to_string()),
            offset: 0,
        },
    )
    .await
    .expect("open");

    put.write(&[1u8; 3]).await.expect("part of the body");

    // Finishing the stream here would tell the machine the file is complete,
    // and it would store three bytes under an id a prompt then references as
    // though it were the whole photo.
    let ended = put.finish().await;

    machine.abort();

    assert!(ended.is_err(), "a short upload was finished as a whole file");
}

#[tokio::test]
async fn a_resumed_upload_owes_only_what_the_machine_has_not_got() {
    let wire = Loopback::connect().await.expect("loopback");
    // Six of the eight bytes already reached disk on a previous attempt.
    let machine = tokio::spawn(serve_put(wire.server.clone(), 6));

    let (ready, mut put) = Put::open(
        &wire.client,
        PutSpec {
            name: "photo.png".to_string(),
            len: 8,
            sha256: Sha256("00".to_string()),
            // What this client proposed, which is a proposal and not a decision.
            offset: 0,
        },
    )
    .await
    .expect("open");

    assert_eq!(ready.offset, 6);

    put.write(&[1u8; 2]).await.expect("the remainder");

    let result = put.finish().await.expect("finish");

    machine.await.ok();

    assert_eq!(result.asset.as_str(), AssetId::mint("stored").as_str());
}

#[tokio::test]
async fn a_digest_keeps_the_leading_zero_of_every_byte() {
    // The known vector for the empty input. It is here for one reason: hex
    // written with `{:x}` rather than `{:02x}` drops the leading zero of any
    // byte below 16, so the digest is short, and it is short only for some
    // inputs. Both ends compute this and compare, so a client that formats it
    // differently refuses files that are perfectly intact.
    let empty = Digest::of(&[]);

    assert_eq!(
        empty.as_str(),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(empty.as_str().len(), 64);
}

#[test]
fn a_download_that_never_got_a_head_is_not_retried() {
    let mut retry = Retry::new();

    // Nothing was ever served and nothing is on disk. That is a machine which
    // is not there, or an asset that is gone - and asking it eleven more times
    // from a phone in somebody's pocket spends battery on an answer that will
    // not change.
    assert!(matches!(retry.after(false, false), Next::GiveUp));
}

#[test]
fn a_locked_phone_does_not_spend_a_download_s_attempts() {
    let mut retry = Retry::new();

    // Every dial is refused while this app is locked, and being refused is not
    // the machine failing. Counting these would exhaust the budget in seconds
    // and report a download as failed for the one reason that is certain to
    // clear itself the moment somebody picks the phone up.
    for _ in 0..(Retry::ATTEMPTS * 4) {
        assert!(matches!(retry.after(true, true), Next::Wait(_)));
    }

    assert!(matches!(retry.after(true, false), Next::Wait(_)));
}

#[test]
fn a_phone_nobody_comes_back_to_is_eventually_given_up_on() {
    let mut retry = Retry::new();
    let mut waited = Duration::ZERO;

    while waited < Retry::LOCKED_PATIENCE {
        match retry.after(true, true) {
            Next::Wait(pause) => waited += pause,
            Next::GiveUp => break,
        }
    }

    assert!(
        matches!(retry.after(true, true), Next::GiveUp),
        "a download waited on a locked phone for ever"
    );
}

#[test]
fn the_wait_between_attempts_grows_and_then_stops_growing() {
    let mut retry = Retry::new();
    let mut waits = Vec::new();

    loop {
        match retry.after(true, false) {
            Next::Wait(pause) => waits.push(pause),
            Next::GiveUp => break,
        }
    }

    // Bounded, so a transfer cannot retry for ever, and capped, so the tail of
    // a long wait does not double into hours.
    assert_eq!(waits.len(), (Retry::ATTEMPTS - 1) as usize);
    assert!(waits[0] < waits[1]);
    assert!(waits.iter().all(|pause| *pause <= Retry::LONGEST));
    assert_eq!(*waits.last().expect("a wait"), Retry::LONGEST);
}

#[test]
fn a_partial_is_cut_back_to_the_offset_the_machine_chose() {
    let dir = tempfile::tempdir().expect("a directory");
    let asset = AssetId::mint("apk");
    let whole: Vec<u8> = (0..10u8).collect();

    // A first attempt that reached ten bytes, six of which are rubbish - the
    // tail of a write that was cut off part way through.
    let partial = Partial::new(dir.path(), &asset);
    let mut first = partial.resume(0).expect("a fresh partial");
    first.write(&whole[..4]).expect("the good half");
    first.write(&[0xFFu8; 6]).expect("the half that was lost");
    first.finish().expect("flush");

    assert_eq!(partial.bytes(), 10);

    // The machine vouches for four. Keeping the other six because this phone
    // happens to hold them is how a file that passes its length check fails to
    // install: the bytes are there, and six of them are wrong.
    let mut second = partial.resume(4).expect("resume");

    assert_eq!(second.bytes(), 4);

    second.write(&whole[4..]).expect("the remainder");

    let hashed = second.finish().expect("finish");

    assert_eq!(std::fs::read(partial.path()).expect("read back"), whole);
    assert_eq!(hashed, Digest::of(&whole));
}

#[test]
fn a_machine_that_starts_past_what_this_phone_holds_is_refused() {
    let dir = tempfile::tempdir().expect("a directory");
    let partial = Partial::new(dir.path(), &AssetId::mint("apk"));

    let mut first = partial.resume(0).expect("a fresh partial");
    first.write(&[1u8, 2, 3]).expect("three bytes");
    first.finish().expect("flush");

    // Appending at nine would leave bytes four through eight as whatever the
    // filesystem puts in a hole, under a file whose length looks right.
    let refused = partial.resume(9);

    assert!(
        refused.is_err(),
        "a download was allowed to append past the end of what it holds"
    );
}

#[test]
fn an_asset_id_shaped_like_a_path_cannot_escape_the_download_directory() {
    let dir = tempfile::tempdir().expect("a directory");
    let inside = dir.path().join("downloads");

    // `AssetId::parse` checks the prefix and that something follows it, and
    // nothing else. The id arrives from a peer, so spelling a partial's name
    // with it directly writes wherever the peer says.
    let hostile = AssetId::parse("as_../../escaped").expect("an id");
    let partial = Partial::new(&inside, &hostile);

    partial.resume(0).expect("a fresh partial").finish().expect("flush");

    assert_eq!(partial.path().parent(), Some(inside.as_path()));
    assert!(partial.path().exists());
    assert!(!dir.path().join("escaped.part").exists());
}

#[test]
fn a_partial_nobody_came_back_for_is_swept() {
    let dir = tempfile::tempdir().expect("a directory");
    let partial = Partial::new(dir.path(), &AssetId::mint("apk"));

    partial.resume(0).expect("a fresh partial").finish().expect("flush");

    Partial::sweep(dir.path(), Duration::from_secs(3600));

    assert!(
        partial.path().exists(),
        "a partial written a moment ago is somebody's paused download"
    );

    Partial::sweep(dir.path(), Duration::ZERO);

    assert!(!partial.path().exists());
}
