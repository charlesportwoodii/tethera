use tethera_client_core::endpoint::ClientEndpoint;
use tethera_client_core::pairing::{Begun, PairingSession};
use tethera_common::protocol::handshake::{ClientInfo, Platform, RefuseReason};
use tethera_common::structs::client::BeginOutcome;

use super::fake::{Answer, FakeMachine};

fn a_client() -> ClientInfo {
    ClientInfo {
        app_version: "0.1.0".to_string(),
        platform: Platform::Ios,
        install_id: "3f9a2c".to_string(),
    }
}

async fn an_endpoint() -> ClientEndpoint {
    ClientEndpoint::bind_local().await.expect("bind")
}

// The endpoint is passed in rather than made here. Dropping an endpoint closes
// every connection it opened, so a helper that owned one would hand back a
// session whose stream was already dead - which reads as LinkLost on the first
// submit and looks like a protocol bug. The real app holds one endpoint for the
// life of the process, and these tests hold one for the life of the test.
async fn begin_against(endpoint: &ClientEndpoint, machine: &FakeMachine) -> Begun {
    PairingSession::begin(
        endpoint,
        &machine.offer_uri(),
        a_client(),
        "phone".to_string(),
    )
    .await
    .expect("begin")
}

fn an_open_window() -> Answer {
    Answer::Enroll {
        code: "483920".to_string(),
        attempts: 5,
        expires_in_ms: 60_000,
    }
}

#[tokio::test]
async fn an_open_window_offers_the_machine_and_keeps_the_session() {
    let machine = FakeMachine::start(an_open_window()).await;

    let endpoint = an_endpoint().await;
    let begun = begin_against(&endpoint, &machine).await;

    match begun.outcome {
        BeginOutcome::Found(found) => {
            assert_eq!(found.server.label, FakeMachine::LABEL);
            assert_eq!(found.code_length, 6);
            assert_eq!(found.expires_in_ms, 60_000);
        }
        other => panic!("expected Found, got {other:?}"),
    }

    assert!(
        begun.session.is_some(),
        "the enrolment stream must stay open for the code"
    );
}

// The offer's id is a claim by whoever printed the QR; ServerInfo.id is proved
// by QUIC TLS, because the endpoint id is the public key. When they disagree
// you scanned one machine and reached another, and pairing anyway would enrol
// this device somewhere it was never shown.
#[tokio::test]
async fn a_machine_that_names_a_different_server_id_is_refused() {
    let machine = FakeMachine::start_as(an_open_window(), Some("sv_somewhere_else")).await;

    let endpoint = an_endpoint().await;
    let begun = begin_against(&endpoint, &machine).await;

    assert!(matches!(begun.outcome, BeginOutcome::IdMismatch { .. }));
    assert!(begun.session.is_none(), "a refused attempt parks no session");
}

#[tokio::test]
async fn a_machine_that_already_knows_this_device_reports_already_paired() {
    let machine = FakeMachine::start(Answer::Session).await;

    let endpoint = an_endpoint().await;
    let begun = begin_against(&endpoint, &machine).await;

    match begun.outcome {
        BeginOutcome::AlreadyPaired(entry) => {
            assert_eq!(entry.server.label, FakeMachine::LABEL);
            assert_eq!(entry.endpoint_id, machine.endpoint_id());
        }
        other => panic!("expected AlreadyPaired, got {other:?}"),
    }

    assert!(begun.session.is_none());
}

#[tokio::test]
async fn no_open_window_reports_the_window_closed() {
    let machine = FakeMachine::start(Answer::Refuse(RefuseReason::PairingWindowClosed)).await;

    let endpoint = an_endpoint().await;
    let begun = begin_against(&endpoint, &machine).await;

    assert!(matches!(begun.outcome, BeginOutcome::WindowClosed));
}

#[tokio::test]
async fn a_revoked_refusal_is_reported_as_revoked() {
    let machine = FakeMachine::start(Answer::Refuse(RefuseReason::Revoked)).await;

    let endpoint = an_endpoint().await;
    let begun = begin_against(&endpoint, &machine).await;

    assert!(matches!(begun.outcome, BeginOutcome::Revoked));
}

#[tokio::test]
async fn no_common_version_is_its_own_outcome() {
    let machine = FakeMachine::start(Answer::Refuse(RefuseReason::NoCommonVersion)).await;

    let endpoint = an_endpoint().await;
    let begun = begin_against(&endpoint, &machine).await;

    assert!(matches!(begun.outcome, BeginOutcome::NoCommonVersion));
}

// The machine is full. "Try again shortly" and "you are not paired" send a
// person to entirely different places, so a transport close must never read as
// a refusal.
#[tokio::test]
async fn a_close_at_capacity_is_not_read_as_a_refusal() {
    let machine = FakeMachine::start(Answer::Close {
        code: 1,
        reason: b"at capacity",
    })
    .await;

    let endpoint = an_endpoint().await;
    let begun = begin_against(&endpoint, &machine).await;

    assert!(matches!(begun.outcome, BeginOutcome::AtCapacity));
}

// An unknown code means the machine is newer than this client, which is a
// different sentence again. Folding it into AtCapacity would tell somebody to
// wait for something that will never change.
#[tokio::test]
async fn an_unknown_close_code_is_reported_as_itself() {
    let machine = FakeMachine::start(Answer::Close {
        code: 47,
        reason: b"something later",
    })
    .await;

    let endpoint = an_endpoint().await;
    let begun = begin_against(&endpoint, &machine).await;

    assert!(matches!(
        begun.outcome,
        BeginOutcome::ClosedByMachine { code: 47 }
    ));
}

#[tokio::test]
async fn a_uri_that_is_not_a_pairing_offer_is_an_error_not_an_outcome() {
    let endpoint = an_endpoint().await;

    let result = PairingSession::begin(
        &endpoint,
        "https://example.com/pair?s=sv_atlas",
        a_client(),
        "phone".to_string(),
    )
    .await;

    assert!(result.is_err(), "a foreign scheme is malformed input");
}

// The offer carries no endpoint id, so there is nothing to dial. This is a real
// input - a machine whose server has never run - and it must say so rather than
// fail obscurely.
#[tokio::test]
async fn an_offer_with_no_endpoint_id_is_an_error() {
    let endpoint = an_endpoint().await;

    let result = PairingSession::begin(
        &endpoint,
        "tethera://pair?s=sv_atlas",
        a_client(),
        "phone".to_string(),
    )
    .await;

    assert!(result.is_err());
}

use tethera_common::structs::client::PairOutcome;

async fn a_session_against(endpoint: &ClientEndpoint, machine: &FakeMachine) -> PairingSession {
    begin_against(endpoint, machine)
        .await
        .session
        .expect("an open session")
}

#[tokio::test]
async fn the_right_code_pairs_and_returns_an_entry() {
    let machine = FakeMachine::start(an_open_window()).await;
    let endpoint = an_endpoint().await;
    let mut session = a_session_against(&endpoint, &machine).await;

    match session.submit("483920").await.expect("submit") {
        PairOutcome::Paired(entry) => {
            assert_eq!(entry.server.label, FakeMachine::LABEL);
            assert_eq!(entry.endpoint_id, machine.endpoint_id());
        }
        other => panic!("expected Paired, got {other:?}"),
    }
}

// The whole reason the stream is held open. Somebody mistyping six digits is
// the common case, and re-dialling per guess would defeat the machine's budget.
#[tokio::test]
async fn a_wrong_code_reports_attempts_left_and_the_session_still_works() {
    let machine = FakeMachine::start(an_open_window()).await;
    let endpoint = an_endpoint().await;
    let mut session = a_session_against(&endpoint, &machine).await;

    match session.submit("000000").await.expect("first submit") {
        PairOutcome::WrongCode { attempts_left } => assert_eq!(attempts_left, 4),
        other => panic!("expected WrongCode, got {other:?}"),
    }

    match session.submit("483920").await.expect("second submit") {
        PairOutcome::Paired(_) => {}
        other => panic!("the session must survive a wrong code, got {other:?}"),
    }
}

// Zero is terminal, and the copy above it must not claim five wrong guesses:
// the machine sends zero both when the attempts are spent and when no window
// was open, and a client cannot tell which.
#[tokio::test]
async fn the_last_attempt_reports_the_window_as_spent() {
    let machine = FakeMachine::start(Answer::Enroll {
        code: "483920".to_string(),
        attempts: 1,
        expires_in_ms: 60_000,
    })
    .await;
    let endpoint = an_endpoint().await;
    let mut session = a_session_against(&endpoint, &machine).await;

    match session.submit("000000").await.expect("submit") {
        PairOutcome::WindowSpent => {}
        other => panic!("expected WindowSpent, got {other:?}"),
    }
}

// The code is read off a screen and typed on a phone, which adds spaces and
// changes case. Refusing a correctly-read code over whitespace would be this
// client's fault, not the person's.
#[tokio::test]
async fn a_code_typed_with_spaces_still_pairs() {
    let machine = FakeMachine::start(an_open_window()).await;
    let endpoint = an_endpoint().await;
    let mut session = a_session_against(&endpoint, &machine).await;

    match session.submit("  483920 ").await.expect("submit") {
        PairOutcome::Paired(_) => {}
        other => panic!("expected Paired, got {other:?}"),
    }
}
