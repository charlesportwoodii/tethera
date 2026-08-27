use tethera_client_core::book::ServerBook;
use tethera_common::protocol::capability::CapabilitySet;
use tethera_common::protocol::handshake::{DeviceRecord, ServerInfo};
use tethera_common::structs::client::ServerEntry;
use tethera_common::structs::ids::{DeviceId, ServerId};
use tethera_common::structs::primitives::Timestamp;

fn an_entry(server_id: &str, label: &str, relay: Option<&str>) -> ServerEntry {
    ServerEntry {
        server: ServerInfo {
            id: ServerId::parse(server_id).expect("a valid server id"),
            label: label.to_string(),
            app_version: "0.1.0".to_string(),
            os: "windows".to_string(),
            arch: "x86_64".to_string(),
        },
        endpoint_id: "555bfc38".to_string(),
        relay: relay.map(str::to_string),
        direct_addrs: vec!["10.57.2.4:57909".to_string()],
        device: DeviceRecord {
            id: DeviceId::parse("dv_phone").expect("a valid device id"),
            name: "phone".to_string(),
            paired_at: Timestamp(1),
        },
        capabilities: CapabilitySet::new(),
        last_seen_at: None,
        conversations: Vec::new(),
    }
}

fn a_path(dir: &tempfile::TempDir) -> std::path::PathBuf {
    dir.path().join(ServerBook::FILE_NAME)
}

#[test]
fn an_absent_file_opens_an_empty_book() {
    let dir = tempfile::tempdir().expect("tempdir");

    let book = ServerBook::open(a_path(&dir)).expect("open");

    assert!(book.entries().is_empty());
    assert!(!book.install_id().is_empty());
}

// The install id identifies this installation across re-pairings, so a value
// that changed on every launch would defeat its only purpose.
#[test]
fn the_install_id_survives_a_reopen() {
    let dir = tempfile::tempdir().expect("tempdir");

    let first = ServerBook::open(a_path(&dir)).expect("open");
    let id = first.install_id();
    first
        .upsert(an_entry("sv_atlas", "atlas", None))
        .expect("upsert forces a write");

    let second = ServerBook::open(a_path(&dir)).expect("reopen");

    assert_eq!(second.install_id(), id);
}

// Re-pairing a machine must replace its row. Appending would leave a phone
// showing the same machine twice, one of them with dial details that no longer
// work.
#[test]
fn upserting_the_same_server_id_replaces_rather_than_appends() {
    let dir = tempfile::tempdir().expect("tempdir");
    let book = ServerBook::open(a_path(&dir)).expect("open");

    book.upsert(an_entry("sv_atlas", "atlas", None))
        .expect("first");
    book.upsert(an_entry(
        "sv_atlas",
        "atlas renamed",
        Some("https://use1-1.example/"),
    ))
    .expect("second");

    let entries = book.entries();

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].server.label, "atlas renamed");
    assert_eq!(entries[0].relay.as_deref(), Some("https://use1-1.example/"));
}

#[test]
fn two_different_servers_both_survive() {
    let dir = tempfile::tempdir().expect("tempdir");
    let book = ServerBook::open(a_path(&dir)).expect("open");

    book.upsert(an_entry("sv_atlas", "atlas", None))
        .expect("first");
    book.upsert(an_entry("sv_bramble", "bramble", None))
        .expect("second");

    assert_eq!(book.entries().len(), 2);
}

#[test]
fn forgetting_removes_the_row_and_reports_whether_it_was_there() {
    let dir = tempfile::tempdir().expect("tempdir");
    let book = ServerBook::open(a_path(&dir)).expect("open");
    book.upsert(an_entry("sv_atlas", "atlas", None))
        .expect("upsert");

    let atlas = ServerId::parse("sv_atlas").expect("valid");
    let keel = ServerId::parse("sv_keel").expect("valid");

    assert!(book.forget(&atlas).expect("forget atlas"));
    assert!(!book.forget(&keel).expect("forget an unknown machine"));
    assert!(book.entries().is_empty());
}

// A parse failure must not present a paired person with the first-launch
// screen and invite them to re-pair machines that already know them. An absent
// file is the ordinary first launch; an unreadable one is a fault.
#[test]
fn an_unreadable_file_is_an_error_rather_than_an_empty_book() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = a_path(&dir);
    std::fs::write(&path, b"{ this is not json").expect("write");

    let result = ServerBook::open(path.clone());

    assert!(result.is_err(), "a corrupt book must not open empty");
    assert!(
        std::fs::read(&path)
            .expect("still readable")
            .starts_with(b"{ this"),
        "the corrupt file must not be overwritten"
    );
}

/// A book written by an older build must still open.
///
/// `ServerEntry` embeds `Conversation`, which is a wire type and gains fields
/// whenever the protocol does. This exact shape - a cached conversation missing
/// a field a newer build requires - stopped the app starting on a device:
/// `ServerBook::open` failed, the Tauri setup hook failed with it, and the
/// process exited before drawing anything. The machines are the irreplaceable
/// part; the conversations are a cache the next sweep refills.
#[test]
fn a_cached_conversation_from_an_older_build_does_not_stop_the_book_opening() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = a_path(&dir);

    // One paired machine, and a cached conversation with a field missing.
    let entry = serde_json::to_value(an_entry("sv_atlas", "atlas", None)).expect("entry");
    let mut raw = serde_json::json!({
        "install_id": "install-one",
        "servers": [entry],
    });

    raw["servers"][0]["conversations"] = serde_json::json!([{
        "id": "cv_stale",
        "profile": "claude-code",
        "profile_label": "Claude Code",
        "title": "written by an older build",
    }]);

    std::fs::write(&path, serde_json::to_vec(&raw).expect("json")).expect("write");

    let book = ServerBook::open(path).expect("a stale cache must not stop the book opening");
    let entries = book.entries();

    assert_eq!(entries.len(), 1, "the paired machine must survive");
    assert_eq!(entries[0].server.label, "atlas");
    assert!(
        entries[0].conversations.is_empty(),
        "the unreadable cache is dropped rather than guessed at"
    );
}
