use iroh::SecretKey;
use tethera_transport::endpoint::{EndpointConfig, TetheraEndpoint};

// A port a router forward can name. Chosen high and uncommon so the assertion is
// about our code rather than about what else happens to be listening.
const PINNED: u16 = 47821;

// These two bind through the real `bind`, not the test-only local one, because
// the pinning happens there. `presets::N0` starts relay discovery in the
// background as a side effect; neither assertion depends on it.

#[tokio::test]
async fn a_pinned_port_is_the_port_that_gets_bound() {
    let endpoint = TetheraEndpoint::bind(
        EndpointConfig::new(SecretKey::generate()).with_bind_port(PINNED),
    )
    .await
    .expect("bind");

    let bound: Vec<u16> = endpoint
        .inner()
        .bound_sockets()
        .into_iter()
        .filter(|addr| addr.is_ipv4())
        .map(|addr| addr.port())
        .collect();

    assert!(
        bound.contains(&PINNED),
        "expected an IPv4 socket on {PINNED}, got {bound:?}"
    );
}

// The property that makes a port forward worth having. Falling back to an
// ephemeral port here would leave the forward pointing at nothing: direct
// connections would quietly stop being established, everything would keep
// working over the relay, and nothing would say why it got slower.
#[tokio::test]
async fn a_port_already_in_use_fails_rather_than_moving_to_another() {
    let held = TetheraEndpoint::bind(
        EndpointConfig::new(SecretKey::generate()).with_bind_port(PINNED + 1),
    )
    .await
    .expect("the first bind holds the port");

    let second = TetheraEndpoint::bind(
        EndpointConfig::new(SecretKey::generate()).with_bind_port(PINNED + 1),
    )
    .await;

    assert!(second.is_err(), "the second bind must not silently relocate");

    drop(held);
}
