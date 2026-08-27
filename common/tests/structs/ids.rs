use tethera_common::structs::ids::{ConversationId, PaneId};

#[test]
fn a_prefixed_id_parses_only_with_its_own_prefix() {
    assert!(PaneId::parse("pn_a1b2c3").is_some());
    assert!(PaneId::parse("cv_a1b2c3").is_none());
    assert!(PaneId::parse("a1b2c3").is_none());
}

// A bare prefix identifies nothing, and accepting it would let an empty string
// resolve to whatever the first row happens to be.
#[test]
fn a_prefix_with_nothing_after_it_is_not_an_id() {
    assert!(PaneId::parse("pn_").is_none());
}

#[test]
fn an_id_keeps_its_whole_string_including_the_prefix() {
    assert_eq!(
        ConversationId::parse("cv_9f21").expect("valid").as_str(),
        "cv_9f21"
    );
}

#[test]
fn minting_prepends_the_prefix() {
    assert_eq!(ConversationId::mint("9f21").as_str(), "cv_9f21");
}

#[test]
fn an_id_round_trips_through_postcard() {
    let id = PaneId::parse("pn_a1b2c3").expect("valid");
    let bytes = postcard::to_stdvec(&id).expect("encode");

    assert_eq!(postcard::from_bytes::<PaneId>(&bytes).expect("decode"), id);
}
