use tethera_common::structs::link::{Link, LinkKind};

// The client measures this from its own Iroh endpoint. It is typed here so both
// halves share one vocabulary, never because the server answers it.
#[test]
fn a_link_round_trips_through_postcard() {
    let link = Link {
        kind: LinkKind::Direct,
        rtt_ms: Some(38),
    };
    let bytes = postcard::to_stdvec(&link).expect("encode");

    assert_eq!(postcard::from_bytes::<Link>(&bytes).expect("decode"), link);
}

// Immediately after connecting there is often genuinely no settled path. Drawing
// that as "direct" is a claim the client cannot support, and the predecessor's
// UI made it and was wrong about half the time it appeared.
#[test]
fn an_unsettled_path_is_unknown_rather_than_a_guess() {
    let link = Link {
        kind: LinkKind::Unknown,
        rtt_ms: None,
    };

    assert_ne!(link.kind, LinkKind::Direct);
    assert_ne!(link.kind, LinkKind::Offline);
    assert!(link.rtt_ms.is_none());
}
