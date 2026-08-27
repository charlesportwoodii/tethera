use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub enum PushPlatform {
    /// Firebase, which reaches iOS through APNs behind it.
    Fcm,
}

/// When a machine should reach for a phone that is not looking at it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct NotifyPolicy {
    /// An agent is waiting on a person.
    pub on_blocked: bool,
    pub on_done: bool,
    pub on_failed: bool,
}
