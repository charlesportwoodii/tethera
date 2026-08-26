use crate::errors::TetheraError;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub enum DeviceState {
    Pending,
    Active,
    Revoked,
    Banned,
}

impl DeviceState {
    // Neither Revoked nor Banned returns directly to Active. That is a
    // security invariant, not a convenience: a device that could be
    // re-approved without a fresh pairing code would make revocation
    // advisory.
    //
    // A ban is reversible, but only back to Pending. The devices being banned
    // belong to the operator running the tool, so an unappealable ban is a
    // foot-gun rather than a protection; lifting one returns the device to the
    // start of pairing, where it needs a new code like any other.
    pub fn can_transition_to(&self, next: DeviceState) -> bool {
        matches!(
            (self, next),
            (Self::Pending, Self::Active)
                | (Self::Pending, Self::Revoked)
                | (Self::Pending, Self::Banned)
                | (Self::Active, Self::Revoked)
                | (Self::Active, Self::Banned)
                | (Self::Revoked, Self::Banned)
                | (Self::Banned, Self::Pending)
        )
    }

    // The stored vocabulary lives beside the parser so a writer cannot spell a
    // state differently from the reader that has to recognise it.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Active => "active",
            Self::Revoked => "revoked",
            Self::Banned => "banned",
        }
    }
}

impl FromStr for DeviceState {
    type Err = TetheraError;

    // An unrecognised value is an error rather than a default. Mapping it to
    // Pending would silently downgrade a banned device to an approvable one.
    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        match raw {
            "pending" => Ok(Self::Pending),
            "active" => Ok(Self::Active),
            "revoked" => Ok(Self::Revoked),
            "banned" => Ok(Self::Banned),
            other => Err(TetheraError::UnknownDeviceState(other.to_string())),
        }
    }
}
