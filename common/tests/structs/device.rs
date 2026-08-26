use std::str::FromStr;
use tethera_common::structs::device::DeviceState;

#[test]
fn a_pending_device_can_be_approved() {
    assert!(DeviceState::Pending.can_transition_to(DeviceState::Active));
}

#[test]
fn a_revoked_device_can_never_return_to_active() {
    assert!(!DeviceState::Revoked.can_transition_to(DeviceState::Active));
}

#[test]
fn a_ban_is_lifted_only_back_to_pending() {
    assert!(
        DeviceState::Banned.can_transition_to(DeviceState::Pending),
        "a ban must be reversible"
    );

    for next in [DeviceState::Active, DeviceState::Revoked] {
        assert!(
            !DeviceState::Banned.can_transition_to(next),
            "lifting a ban must not reach {next:?} directly; it must re-pair"
        );
    }
}

#[test]
fn a_state_never_transitions_to_itself() {
    for state in [
        DeviceState::Pending,
        DeviceState::Active,
        DeviceState::Revoked,
        DeviceState::Banned,
    ] {
        assert!(!state.can_transition_to(state), "{state:?} to itself");
    }
}

#[test]
fn every_state_parses_back_from_the_value_it_writes() {
    for state in [
        DeviceState::Pending,
        DeviceState::Active,
        DeviceState::Revoked,
        DeviceState::Banned,
    ] {
        assert_eq!(
            DeviceState::from_str(state.as_str()).expect("its own value must parse"),
            state
        );
    }
}

#[test]
fn an_unknown_state_is_an_error_rather_than_a_downgrade_to_pending() {
    assert!(DeviceState::from_str("Active").is_err());
    assert!(DeviceState::from_str("quarantined").is_err());
    assert!(DeviceState::from_str("").is_err());
}
