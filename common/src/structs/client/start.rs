use crate::structs::conversation::Conversation;
use crate::structs::ids::PaneId;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Every way starting a session can end well enough to draw a screen for.
///
/// The second variant is not a failure. The pane is open and the harness is
/// running in it, but something at the machine is holding it before its first
/// record, so there is no conversation to navigate to. Reporting that as a
/// failure sends somebody looking for a fault that is not there; reporting it as
/// success sends them to a transcript that will never fill.
///
/// A real failure - an unknown profile, a directory that does not exist, a
/// machine that refused - is still an `Err`, and stays one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub enum StartOutcome {
    Started(Conversation),
    /// The agent is alive in `pane` and has begun no conversation. Nothing on
    /// the wire distinguishes a directory it has not been trusted with from a
    /// sign-in or an onboarding screen, and nothing needs to: the answer is the
    /// same, and it is given at the machine.
    AwaitingAgent { pane: PaneId },
}
