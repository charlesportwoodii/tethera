use tethera_common::structs::pairing::{PairingCode, PairingOffer};

#[test]
fn an_offer_survives_a_uri_round_trip() {
    let offer = PairingOffer::new(
        "server-abc".to_string(),
        Some("k51qzi5uqu5dkwkqm42v9j9kqcam2jiuvloi16g72i4i4amoo2m8u3ol3mqu6s".to_string()),
        Some("https://use1-1.relay.tethera.net/".to_string()),
        vec!["192.168.1.10:41641".to_string(), "10.0.0.4:41641".to_string()],
        Some("atlas".to_string()),
    );

    let parsed = PairingOffer::from_uri(&offer.to_uri()).expect("round trip");

    assert_eq!(parsed.server_id, offer.server_id);
    assert_eq!(parsed.endpoint_id, offer.endpoint_id);
    assert_eq!(parsed.direct_addrs, offer.direct_addrs);
    assert_eq!(parsed.relay, offer.relay);
    assert_eq!(parsed.label, offer.label);
}

#[test]
fn a_uri_without_the_required_server_id_is_rejected() {
    assert!(PairingOffer::from_uri("tethera://pair?n=abc").is_err());
}

#[test]
fn a_uri_with_the_wrong_scheme_is_rejected() {
    assert!(PairingOffer::from_uri("https://example.com/pair?s=abc").is_err());
}

#[test]
fn an_offer_with_no_optional_fields_still_round_trips() {
    let offer = PairingOffer::new("only-an-id".to_string(), None, None, Vec::new(), None);

    let parsed = PairingOffer::from_uri(&offer.to_uri()).expect("round trip");

    assert_eq!(parsed.server_id, "only-an-id");
    assert_eq!(parsed.endpoint_id, None);
    assert!(parsed.direct_addrs.is_empty());
    assert!(parsed.relay.is_none());
    assert!(parsed.label.is_none());
}

#[test]
fn a_pairing_code_verifies_only_the_exact_code() {
    let code = PairingCode::from_plaintext("482913");

    assert!(code.verify("482913"));
    assert!(!code.verify("482914"));
    assert!(!code.verify("48291"));
    assert!(!code.verify(""));
}

#[test]
fn a_uri_with_the_wrong_host_is_rejected() {
    assert!(PairingOffer::from_uri("tethera://revoke?s=abc").is_err());
    assert!(PairingOffer::from_uri("tethera://anything?s=abc").is_err());
}

// The offer is an address, not a credential. It stays valid indefinitely and is
// not sensitive, which is what removes the predecessor's trap where the key in
// the QR rotated and an older photograph failed with an opaque 403.
#[test]
fn an_offer_carries_no_secret() {
    let uri = PairingOffer::new(
        "sv_a1".to_string(),
        None,
        None,
        Vec::new(),
        Some("atlas".to_string()),
    )
    .to_uri();

    assert!(!uri.contains("k="));
    assert!(!uri.contains("token"));
}

// An unknown key is ignored rather than refused, so a newer machine's QR still
// pairs an older phone.
#[test]
fn an_unknown_query_key_does_not_stop_a_pairing() {
    let parsed = PairingOffer::from_uri("tethera://pair?s=sv_a1&l=atlas&something_new=1")
        .expect("parse");

    assert_eq!(parsed.label.as_deref(), Some("atlas"));
}

// A machine with no label configured has no name, and an empty string would
// render as a name that happens to be invisible.
#[test]
fn a_machine_with_no_label_reports_none_rather_than_an_empty_name() {
    let parsed = PairingOffer::from_uri("tethera://pair?s=sv_a1").expect("parse");

    assert!(parsed.label.is_none());
}
