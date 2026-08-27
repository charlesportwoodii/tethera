use tethera_common::protocol::transfer::{FetchHead, FetchSpec, PutReady, PutResult, PutSpec};
use tethera_common::structs::ids::AssetId;
use tethera_common::structs::primitives::Sha256;

#[test]
fn a_fetch_spec_carries_the_offset_to_resume_from() {
    let spec = FetchSpec {
        asset: AssetId::parse("as_9f21ab").expect("valid"),
        offset: 4096,
    };
    let bytes = postcard::to_stdvec(&spec).expect("encode");

    assert_eq!(
        postcard::from_bytes::<FetchSpec>(&bytes).expect("decode"),
        spec
    );
}

// `len` is the total size of the asset, not the number of bytes about to be
// sent. A client reading it as "bytes remaining" draws a progress bar that
// finishes early on every resumed transfer.
#[test]
fn a_fetch_head_states_the_whole_size_and_where_it_actually_starts() {
    let head = FetchHead {
        len: 1_048_576,
        mime: Some("text/markdown".into()),
        sha256: Sha256("e3b0c442".into()),
        offset: 4096,
    };

    assert_eq!(head.len, 1_048_576);
    assert_eq!(head.offset, 4096);

    let bytes = postcard::to_stdvec(&head).expect("encode");

    assert_eq!(
        postcard::from_bytes::<FetchHead>(&bytes).expect("decode"),
        head
    );
}

// The client proposes an offset, but only the server knows how much of a
// previous attempt reached disk, so the server's answer is authoritative.
// Without this frame a resumed upload is the client guessing, which corrupts
// the file.
#[test]
fn a_put_ready_names_the_offset_the_server_wants() {
    let ready = PutReady { offset: 8192 };
    let bytes = postcard::to_stdvec(&ready).expect("encode");

    assert_eq!(
        postcard::from_bytes::<PutReady>(&bytes)
            .expect("decode")
            .offset,
        8192
    );
}

// The server's offset is allowed to differ from the client's proposal - that is
// the whole point of the frame, so the two must be independently representable.
#[test]
fn the_server_may_answer_an_offset_the_client_did_not_propose() {
    let proposed = PutSpec {
        name: "screenshot.png".into(),
        len: 240_000,
        sha256: Sha256("e3b0c442".into()),
        offset: 100_000,
    };
    let answered = PutReady { offset: 0 };

    assert_ne!(proposed.offset, answered.offset);
}

#[test]
fn a_put_spec_and_its_result_round_trip() {
    let spec = PutSpec {
        name: "screenshot.png".into(),
        len: 240_000,
        sha256: Sha256("e3b0c442".into()),
        offset: 0,
    };
    let result = PutResult {
        asset: AssetId::parse("as_9f21ab").expect("valid"),
    };

    assert_eq!(
        postcard::from_bytes::<PutSpec>(&postcard::to_stdvec(&spec).expect("e")).expect("d"),
        spec
    );
    assert_eq!(
        postcard::from_bytes::<PutResult>(&postcard::to_stdvec(&result).expect("e")).expect("d"),
        result
    );
}
