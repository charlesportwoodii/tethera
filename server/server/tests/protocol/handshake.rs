use tethera_common::protocol::handshake::{
    CodeFormat, DeviceRecord, EnrollResult, Intent, RefuseReason, ServerHello,
};
use tethera_common::protocol::WireVersion;
use tethera_common::structs::ids::DeviceId;
use tethera_common::structs::primitives::Timestamp;
use tethera_server_lib::protocol::dispatch::{Dispatcher, HandshakeOutcome};
use tethera_server_lib::protocol::ports::{EnrollOffer, Enrolment};

use super::client::Harness;
use super::fakes::{self, FakePorts};

fn a_device() -> DeviceRecord {
    DeviceRecord {
        id: DeviceId::parse("dv_phone").expect("valid"),
        name: "phone".into(),
        paired_at: Timestamp(1),
    }
}

fn an_offer() -> EnrollOffer {
    EnrollOffer {
        server: tethera_common::protocol::handshake::ServerInfo {
            id: tethera_common::structs::ids::ServerId::parse("sv_atlas").expect("valid"),
            label: "atlas".into(),
            app_version: "0.1.0".into(),
            os: "linux".into(),
            arch: "x86_64".into(),
        },
        code_length: 6,
        expires_in_ms: 120_000,
    }
}

// The decision is a pure function of its inputs, so every branch is testable
// with no connection and no database.
#[test]
fn an_unknown_endpoint_with_no_pairing_window_is_refused() {
    let outcome = Dispatcher::<FakePorts>::decide(
        &Enrolment::Unknown,
        None,
        Intent::Enroll,
        WireVersion::SUPPORTED,
    );

    assert_eq!(
        outcome,
        HandshakeOutcome::Refuse(RefuseReason::PairingWindowClosed)
    );
}

#[test]
fn an_unknown_endpoint_asking_for_a_session_is_not_enrolled() {
    let outcome = Dispatcher::<FakePorts>::decide(
        &Enrolment::Unknown,
        Some(an_offer()),
        Intent::Session,
        WireVersion::SUPPORTED,
    );

    assert_eq!(outcome, HandshakeOutcome::Refuse(RefuseReason::NotEnrolled));
}

// Revoked is distinct from unknown. A revoked device that could re-enrol by
// presenting itself as a stranger would make revocation cosmetic.
#[test]
fn a_revoked_endpoint_is_refused_even_with_a_window_open() {
    let outcome = Dispatcher::<FakePorts>::decide(
        &Enrolment::Revoked,
        Some(an_offer()),
        Intent::Enroll,
        WireVersion::SUPPORTED,
    );

    assert_eq!(outcome, HandshakeOutcome::Refuse(RefuseReason::Revoked));
}

// A version problem must never read as an authorisation one: the client tells a
// person to update rather than to re-pair, which is the opposite instruction.
#[test]
fn a_version_mismatch_is_reported_as_a_version_problem_not_an_authorisation_one() {
    let outcome = Dispatcher::<FakePorts>::decide(
        &Enrolment::Unknown,
        None,
        Intent::Session,
        &[WireVersion(999)],
    );

    assert_eq!(
        outcome,
        HandshakeOutcome::Refuse(RefuseReason::NoCommonVersion)
    );
}

#[test]
fn a_known_endpoint_opens_a_session() {
    let outcome = Dispatcher::<FakePorts>::decide(
        &Enrolment::Known(a_device()),
        None,
        Intent::Session,
        WireVersion::SUPPORTED,
    );

    match outcome {
        HandshakeOutcome::Session { device, version } => {
            assert_eq!(device, a_device());
            assert_eq!(
                Some(version),
                WireVersion::SUPPORTED.iter().max().copied(),
                "a session opens at the newest version both sides speak"
            );
        }
        other => panic!("expected a session, got {other:?}"),
    }
}

#[test]
fn an_unknown_endpoint_with_a_window_open_is_offered_enrolment() {
    let outcome = Dispatcher::<FakePorts>::decide(
        &Enrolment::Unknown,
        Some(an_offer()),
        Intent::Enroll,
        WireVersion::SUPPORTED,
    );

    match outcome {
        HandshakeOutcome::EnrollPending { offer, .. } => {
            assert_eq!(offer.code_length, 6);
        }
        other => panic!("expected an enrolment offer, got {other:?}"),
    }
}

// Now over a real connection, so the frames and the decision are proved to agree.
#[tokio::test]
async fn an_open_pairing_window_answers_with_an_enrollment_offer() {
    let harness = Harness::start().await;
    let (answer, _send, _recv) = harness.hello(Intent::Enroll).await;

    match answer {
        ServerHello::EnrollPending {
            server,
            code_length,
            code_format,
            ..
        } => {
            assert_eq!(server.label, "atlas");
            assert_eq!(code_length, fakes::CODE.len() as u8);
            assert_eq!(code_format, CodeFormat::Digits);
        }
        other => panic!("expected an enrolment offer, got {other:?}"),
    }
}

#[tokio::test]
async fn a_wrong_code_is_refused_and_counts_down_the_attempts() {
    let harness = Harness::start().await;
    let (_answer, mut send, mut recv) = harness.hello(Intent::Enroll).await;

    let result = harness.type_code(&mut send, &mut recv, "000000").await;

    match result {
        // Three attempts, one spent.
        EnrollResult::Refused { attempts_left, .. } => assert_eq!(attempts_left, 2),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[tokio::test]
async fn the_right_code_enrolls_the_device_and_returns_the_capability_set() {
    let harness = Harness::start().await;
    let (_answer, mut send, mut recv) = harness.hello(Intent::Enroll).await;

    let result = harness.type_code(&mut send, &mut recv, fakes::CODE).await;

    match result {
        EnrollResult::Accepted {
            device,
            capabilities,
            ..
        } => {
            assert_eq!(device.name, "phone");
            assert!(!capabilities.is_empty());
        }
        other => panic!("expected an acceptance, got {other:?}"),
    }
}

// A client whose version list shares nothing with the server's is refused over
// the wire, not only in the decision function.
#[tokio::test]
async fn a_version_mismatch_is_refused_over_a_real_connection() {
    let harness = Harness::start().await;
    let answer = harness.hello_with_versions(vec![WireVersion(999)]).await;

    assert_eq!(
        answer,
        ServerHello::Refuse(RefuseReason::NoCommonVersion)
    );
}
