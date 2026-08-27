use tethera_common::protocol::handshake::{ClientHello, ClientInfo, Intent, Platform};
use tethera_common::protocol::request::Request;
use tethera_common::protocol::stream::StreamOpen;
use tethera_common::protocol::terminal::AttachSpec;
use tethera_common::protocol::transfer::{FetchSpec, PutSpec};
use tethera_common::protocol::watch::WatchSpec;
use tethera_common::protocol::WireVersion;
use tethera_common::structs::ids::{AssetId, PaneId};
use tethera_common::structs::primitives::Sha256;

fn a_hello() -> StreamOpen {
    StreamOpen::Hello(ClientHello {
        versions: WireVersion::SUPPORTED.to_vec(),
        client: ClientInfo {
            app_version: "0.1.0".into(),
            platform: Platform::Ios,
            install_id: "3f9a2c".into(),
        },
        intent: Intent::Session,
    })
}

// The first frame on a client-opened stream declares what the stream is. After
// that the stream is typed, which is what removes the need for a request id or a
// correlation table of our own.
#[test]
fn every_stream_kind_round_trips() {
    let kinds = vec![
        a_hello(),
        StreamOpen::Rpc(Request::Describe),
        StreamOpen::Watch(WatchSpec::Machine),
        StreamOpen::Attach(AttachSpec {
            pane: PaneId::parse("pn_a1").expect("valid"),
        }),
        StreamOpen::Fetch(FetchSpec {
            asset: AssetId::parse("as_9f21ab").expect("valid"),
            offset: 0,
        }),
        StreamOpen::Put(PutSpec {
            name: "a.png".into(),
            len: 10,
            sha256: Sha256("ab".into()),
            offset: 0,
        }),
    ];

    for open in kinds {
        let bytes = postcard::to_stdvec(&open).expect("encode");

        assert_eq!(
            postcard::from_bytes::<StreamOpen>(&bytes).expect("decode"),
            open
        );
    }
}

// Only Hello may open before the handshake. A new stream kind cannot be added
// without deciding this question, because the test enumerates every variant.
#[test]
fn only_a_hello_may_open_before_the_handshake() {
    assert!(a_hello().is_permitted_before_handshake());

    let after_handshake = vec![
        StreamOpen::Rpc(Request::Describe),
        StreamOpen::Watch(WatchSpec::Machine),
        StreamOpen::Attach(AttachSpec {
            pane: PaneId::parse("pn_a1").expect("valid"),
        }),
        StreamOpen::Fetch(FetchSpec {
            asset: AssetId::parse("as_9f21ab").expect("valid"),
            offset: 0,
        }),
        StreamOpen::Put(PutSpec {
            name: "a.png".into(),
            len: 10,
            sha256: Sha256("ab".into()),
            offset: 0,
        }),
    ];

    assert!(after_handshake
        .iter()
        .all(|open| !open.is_permitted_before_handshake()));
}

// Hello is the first variant, so its discriminant is zero. If that changes, a
// shipped client's handshake decodes as some other stream kind entirely.
#[test]
fn hello_is_the_first_variant() {
    let bytes = postcard::to_stdvec(&a_hello()).expect("encode");

    assert_eq!(bytes[0], 0);
}
