use std::time::Duration;
use tethera_client_core::link::Measure;
use tethera_common::structs::link::LinkKind;
use tethera_transport::stream::testing::Loopback;

// A loopback pair is a direct IP path, so this pins the mapping from "the
// selected path is an IP path" to Direct, and that an rtt is reported once a
// path has settled.
#[tokio::test]
async fn a_loopback_connection_measures_as_direct_with_an_rtt() {
    let pair = Loopback::connect().await.expect("loopback");

    let link = Measure::settled(&pair.client, Duration::from_millis(500)).await;

    assert_eq!(link.kind, LinkKind::Direct);
    assert!(link.rtt_ms.is_some(), "a settled path has a round trip");
}

// Nothing improves on a direct path, so waiting for one to change is time every
// row of a sweep would pay for no answer. A settle window measured in seconds
// must not be spent when the first look already found the best outcome.
#[tokio::test]
async fn a_direct_path_returns_without_spending_the_settle_window() {
    let pair = Loopback::connect().await.expect("loopback");

    let started = std::time::Instant::now();
    let link = Measure::settled(&pair.client, Duration::from_secs(5)).await;

    assert_eq!(link.kind, LinkKind::Direct);
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "settled waited {:?} on an already-direct path",
        started.elapsed()
    );
}

// `LinkKind::Relayed` has no test here, and that is a coverage gap worth
// stating rather than implying. A loopback pair cannot produce a relayed path,
// so the `is_relay` branch is exercised only by the manual run against a real
// machine on a mobile network.
