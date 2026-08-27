use tethera_common::structs::primitives::{Cursor, Fingerprint, Sha256, Timestamp};

#[test]
fn a_timestamp_is_milliseconds_since_the_unix_epoch() {
    assert_eq!(Timestamp(1_766_000_000_000).0, 1_766_000_000_000);
}

// These are strings rather than byte arrays because they cross the Tauri IPC
// boundary as JSON, where a Vec<u8> becomes an array of numbers that is
// unpleasant to hold and easy to compare wrongly.
#[test]
fn the_opaque_wire_values_are_strings() {
    assert_eq!(Cursor("t1:8814".into()).as_str(), "t1:8814");
    assert_eq!(Fingerprint("9f21ab".into()).as_str(), "9f21ab");
    assert_eq!(Sha256("e3b0c442".into()).as_str(), "e3b0c442");
}

#[test]
fn an_opaque_value_round_trips_through_postcard() {
    let cursor = Cursor("t1:8814".into());
    let bytes = postcard::to_stdvec(&cursor).expect("encode");

    assert_eq!(
        postcard::from_bytes::<Cursor>(&bytes).expect("decode"),
        cursor
    );
}
