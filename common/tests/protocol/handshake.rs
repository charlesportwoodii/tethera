use tethera_common::protocol::capability::{self, CapabilityId, CapabilitySet};
use tethera_common::protocol::handshake::{
    ClientHello, ClientInfo, CodeFormat, DeviceRecord, EnrollCode, EnrollResult, Handshake, Intent,
    Platform, RefuseReason, ServerHello, ServerInfo,
};
use tethera_common::protocol::WireVersion;
use tethera_common::structs::ids::{DeviceId, RequestId, ServerId};
use tethera_common::structs::primitives::Timestamp;

fn a_server() -> ServerInfo {
    ServerInfo {
        id: ServerId::parse("sv_a1").expect("valid"),
        label: "atlas".into(),
        app_version: "0.1.0".into(),
        os: "windows".into(),
        arch: "x86_64".into(),
    }
}

fn a_client_hello(intent: Intent) -> ClientHello {
    ClientHello {
        versions: WireVersion::SUPPORTED.to_vec(),
        client: ClientInfo {
            app_version: "0.1.0".into(),
            platform: Platform::Ios,
            install_id: "3f9a2c".into(),
        },
        intent,
    }
}

#[test]
fn a_client_hello_round_trips_and_offers_every_version_it_speaks() {
    let hello = a_client_hello(Intent::Session);
    let bytes = postcard::to_stdvec(&hello).expect("encode");

    assert_eq!(
        postcard::from_bytes::<ClientHello>(&bytes).expect("decode"),
        hello
    );
    assert_eq!(hello.versions, WireVersion::SUPPORTED.to_vec());
}

// Capabilities are asked, never inferred from a version string: a feature can be
// absent on a machine that is otherwise current.
#[test]
fn a_session_hello_carries_the_capability_set() {
    let mut capabilities = CapabilitySet::new();
    capabilities.insert(CapabilityId::from(capability::TERMINAL_ATTACH));

    let hello = ServerHello::Session {
        version: WireVersion(1),
        server: a_server(),
        capabilities,
        device: DeviceRecord {
            id: DeviceId::parse("dv_b2").expect("valid"),
            name: "phone".into(),
            paired_at: Timestamp(1_766_000_000_000),
        },
    };

    let bytes = postcard::to_stdvec(&hello).expect("encode");

    assert_eq!(
        postcard::from_bytes::<ServerHello>(&bytes).expect("decode"),
        hello
    );
}

// The pairing screen names the machine it found and lays out one cell per
// character before anything is typed. Both facts come from the server, which TLS
// has already authenticated, rather than from the QR, which anyone can
// photograph.
#[test]
fn an_enrollment_offer_names_the_machine_and_the_shape_of_its_code() {
    let pending = ServerHello::EnrollPending {
        request_id: RequestId("req-1".into()),
        expires_in_ms: 120_000,
        server: a_server(),
        code_length: 6,
        code_format: CodeFormat::Digits,
    };

    match &pending {
        ServerHello::EnrollPending {
            server,
            code_length,
            code_format,
            ..
        } => {
            assert_eq!(server.label, "atlas");
            assert_eq!(server.os, "windows");
            assert_eq!(*code_length, 6);
            assert_eq!(*code_format, CodeFormat::Digits);
        }
        _ => panic!("expected an enrollment offer"),
    }

    let bytes = postcard::to_stdvec(&pending).expect("encode");

    assert_eq!(
        postcard::from_bytes::<ServerHello>(&bytes).expect("decode"),
        pending
    );
}

// A connection can fail for only a few reasons, and naming them exhaustively is
// what lets a client explain the failure instead of showing a string.
#[test]
fn every_refusal_reason_round_trips() {
    for reason in [
        RefuseReason::NotEnrolled,
        RefuseReason::PairingWindowClosed,
        RefuseReason::NoCommonVersion,
        RefuseReason::Revoked,
    ] {
        let bytes = postcard::to_stdvec(&reason).expect("encode");

        assert_eq!(
            postcard::from_bytes::<RefuseReason>(&bytes).expect("decode"),
            reason
        );
    }
}

#[test]
fn an_enrollment_code_and_its_result_round_trip() {
    let code = EnrollCode {
        request_id: RequestId("req-1".into()),
        code: "732914".into(),
        device_name: "phone".into(),
    };
    let refused = EnrollResult::Refused {
        reason: RefuseReason::PairingWindowClosed,
        attempts_left: 2,
    };

    assert_eq!(
        postcard::from_bytes::<EnrollCode>(&postcard::to_stdvec(&code).expect("e")).expect("d"),
        code
    );
    assert_eq!(
        postcard::from_bytes::<EnrollResult>(&postcard::to_stdvec(&refused).expect("e"))
            .expect("d"),
        refused
    );
}

// The code is read off a screen and typed on a phone, which adds spaces and
// changes case. Comparing it raw fails a person who typed it correctly.
#[test]
fn a_code_is_compared_upper_cased_and_trimmed() {
    assert_eq!(Handshake::normalize_code("  qk4t "), "QK4T");
    assert_eq!(Handshake::normalize_code("QK4T"), "QK4T");
}

// A refusal and a session are different variants rather than a session with an
// error field, so a client cannot read capabilities off a connection that was
// never established.
#[test]
fn a_refusal_carries_no_session_state() {
    let refused = ServerHello::Refuse(RefuseReason::NotEnrolled);

    match refused {
        ServerHello::Refuse(reason) => assert_eq!(reason, RefuseReason::NotEnrolled),
        _ => panic!("expected a refusal"),
    }
}
