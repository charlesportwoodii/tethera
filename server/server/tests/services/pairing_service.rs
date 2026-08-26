use tethera_common::errors::TetheraError;
use tethera_common::structs::device::DeviceState;
use tethera_common::structs::pairing::PairingCode;
use tethera_server_lib::services::PairingService;

const EXPIRES_AT: i64 = 1000;
const NOW: i64 = 0;
const UNCONSUMED: Option<i64> = None;

#[test]
fn approval_moves_a_pending_device_to_active_when_the_code_matches() {
    let stored = PairingCode::from_plaintext("482913");

    let next = PairingService::approve(DeviceState::Pending, &stored, "482913", EXPIRES_AT, UNCONSUMED, NOW)
        .expect("matching code must approve");

    assert_eq!(next, DeviceState::Active);
}

#[test]
fn a_wrong_code_leaves_the_device_pending() {
    let stored = PairingCode::from_plaintext("482913");

    assert!(
        PairingService::approve(DeviceState::Pending, &stored, "000000", EXPIRES_AT, UNCONSUMED, NOW).is_err()
    );
}

#[test]
fn a_revoked_device_cannot_be_approved_even_with_the_right_code() {
    let stored = PairingCode::from_plaintext("482913");

    assert!(
        PairingService::approve(DeviceState::Revoked, &stored, "482913", EXPIRES_AT, UNCONSUMED, NOW).is_err()
    );
}

#[test]
fn a_banned_device_cannot_be_approved_even_with_the_right_code() {
    let stored = PairingCode::from_plaintext("482913");

    assert!(
        PairingService::approve(DeviceState::Banned, &stored, "482913", EXPIRES_AT, UNCONSUMED, NOW).is_err()
    );
}

#[test]
fn an_expired_code_is_rejected_even_though_the_digest_matches() {
    let stored = PairingCode::from_plaintext("482913");

    let error = PairingService::approve(DeviceState::Pending, &stored, "482913", 1000, UNCONSUMED, 1001)
        .expect_err("an expired code must not approve");

    assert!(matches!(error, TetheraError::PairingCodeExpired));
}

#[test]
fn a_code_expiring_at_this_very_instant_is_already_expired() {
    let stored = PairingCode::from_plaintext("482913");

    let error = PairingService::approve(DeviceState::Pending, &stored, "482913", 1000, UNCONSUMED, 1000)
        .expect_err("the expiry boundary must exclude the instant itself");

    assert!(matches!(error, TetheraError::PairingCodeExpired));
}

#[test]
fn a_code_one_second_from_expiry_still_approves() {
    let stored = PairingCode::from_plaintext("482913");

    let next = PairingService::approve(DeviceState::Pending, &stored, "482913", 1000, UNCONSUMED, 999)
        .expect("a code inside its window must approve");

    assert_eq!(next, DeviceState::Active);
}

#[test]
fn a_consumed_code_is_rejected_even_though_the_digest_matches() {
    let stored = PairingCode::from_plaintext("482913");

    let error = PairingService::approve(
        DeviceState::Pending,
        &stored,
        "482913",
        EXPIRES_AT,
        Some(500),
        NOW,
    )
    .expect_err("a code that has already approved a device must not approve a second");

    assert!(matches!(error, TetheraError::PairingCodeAlreadyUsed));
}

#[test]
fn a_consumed_code_is_rejected_for_every_device_state_it_could_otherwise_approve() {
    let stored = PairingCode::from_plaintext("482913");

    for state in [DeviceState::Pending, DeviceState::Active] {
        assert!(
            PairingService::approve(state, &stored, "482913", EXPIRES_AT, Some(1), NOW).is_err(),
            "a consumed code must not approve a device in state {state:?}"
        );
    }
}

#[test]
fn an_unconsumed_code_inside_its_window_still_approves() {
    let stored = PairingCode::from_plaintext("482913");

    let next = PairingService::approve(DeviceState::Pending, &stored, "482913", EXPIRES_AT, None, NOW)
        .expect("an unconsumed code must still approve");

    assert_eq!(next, DeviceState::Active);
}
