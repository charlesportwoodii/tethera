use tethera_common::protocol::version::WireVersion;

#[test]
fn negotiation_picks_the_highest_version_both_sides_speak() {
    let local = [WireVersion(1), WireVersion(2), WireVersion(3)];
    let remote = [WireVersion(2), WireVersion(3), WireVersion(4)];

    assert_eq!(WireVersion::negotiate(&local, &remote), Some(WireVersion(3)));
}

// A refusal, not a fallback. A link established on a version one side does not
// understand fails mid-session rather than at connect, where it can be reported.
#[test]
fn no_shared_version_is_none_rather_than_a_fallback() {
    assert_eq!(
        WireVersion::negotiate(&[WireVersion(1)], &[WireVersion(7)]),
        None
    );
}

#[test]
fn negotiation_against_an_empty_list_is_none() {
    assert_eq!(WireVersion::negotiate(&[WireVersion(1)], &[]), None);
}

// Three, and only three. A build that offered either older version as well
// would be promising an encoding it cannot produce: version one ended by adding
// struct fields and version two by adding one to `WatchOpen::Machine`, and
// postcard writes a struct variant's fields as positionally as a struct's.
#[test]
fn this_build_speaks_version_three_and_nothing_older() {
    assert_eq!(WireVersion::SUPPORTED, &[WireVersion(3)]);
}

// The refusal an older client actually receives. Silent misdecoding is the
// failure this whole mechanism exists to convert into a legible one.
#[test]
fn a_client_that_only_speaks_version_one_shares_nothing_with_this_build() {
    assert_eq!(
        WireVersion::negotiate(WireVersion::SUPPORTED, &[WireVersion(1)]),
        None
    );
}

// The version an already-installed client speaks, which makes this the refusal
// that actually happens rather than a hypothetical one. It is deliberate: a
// version two client decoding a version three `WatchOpen::Machine` would read
// the layouts field's length prefix as part of the conversation list.
#[test]
fn a_client_that_only_speaks_version_two_shares_nothing_with_this_build() {
    assert_eq!(
        WireVersion::negotiate(WireVersion::SUPPORTED, &[WireVersion(2)]),
        None
    );
}
