use tethera_client_core::endpoint::ClientEndpoint;

use super::fake::{Answer, FakeMachine};

const VALID: &str = "555bfc3855cdecba82293424903294304243b330c42e32c7e5356e8ad2e690bb";

#[test]
fn an_endpoint_id_that_is_not_a_key_is_refused() {
    let result = ClientEndpoint::address("not-a-key", None, &[]);

    assert!(
        result.is_err(),
        "a pairing offer is hostile input and must not be trusted"
    );
}

// One unusable direct address must not lose the others, or a machine with a
// stale entry beside a good one becomes unreachable.
#[test]
fn a_malformed_direct_address_is_skipped_and_the_rest_are_kept() {
    let addrs = vec![
        "not-an-address".to_string(),
        "10.57.2.4:57909".to_string(),
        "104.6.188.20:46436".to_string(),
    ];

    let addr = ClientEndpoint::address(VALID, None, &addrs).expect("address");

    assert_eq!(addr.ip_addrs().count(), 2);
}

// The relay is the single value that makes a cold dial work from a mobile
// network. Dropping a malformed one silently would leave a machine looking
// unreachable with nothing to explain it.
#[test]
fn a_malformed_relay_url_is_an_error_rather_than_a_silent_omission() {
    let result = ClientEndpoint::address(VALID, Some("this is not a url"), &[]);

    assert!(result.is_err());
}

#[test]
fn a_valid_relay_url_is_carried_through() {
    let addr = ClientEndpoint::address(VALID, Some("https://use1-1.relay.n0.iroh.link./"), &[])
        .expect("address");

    assert!(addr.relay_urls().next().is_some());
}

// Both halves of what a resume does, in the order it does them. A reset that
// hands back no resolver, or a hint that hangs, or either one leaving the
// endpoint unable to dial, would turn every resume into the failure it is meant
// to repair.
#[tokio::test]
async fn a_resume_resets_dns_takes_the_hint_and_leaves_the_endpoint_dialable() {
    let machine = FakeMachine::start(Answer::Session).await;
    let endpoint = ClientEndpoint::bind_local().await.expect("bind");

    assert!(
        endpoint.reset_dns(),
        "a bound endpoint must have a resolver to reset"
    );

    assert!(
        endpoint.network_change().await,
        "a bound endpoint must take the hint within the deadline"
    );

    endpoint
        .dial(&machine.endpoint_id(), None, &machine.direct_addrs())
        .await
        .expect("dial after a resume");
}

#[tokio::test]
async fn a_dial_reaches_the_fake_machine() {
    let machine = FakeMachine::start(Answer::Session).await;
    let endpoint = ClientEndpoint::bind_local().await.expect("bind");

    let connection = endpoint
        .dial(&machine.endpoint_id(), None, &machine.direct_addrs())
        .await
        .expect("dial");

    assert_eq!(connection.remote_id().to_string(), machine.endpoint_id());
}
