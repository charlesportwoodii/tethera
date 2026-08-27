use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// What happened to a message somebody typed.
///
/// The second variant is a refusal, but not a fault: a conversation with no pane
/// has nobody to type at, and the answer is to offer a resume rather than to
/// report a send that did not work. Reporting it as an error would tell somebody
/// their message failed when the truth is that the agent is not there yet.
///
/// Everything else — a machine that will not take prompts, a whitespace-only
/// message, an attachment it cannot store — stays an `Err`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub enum SendOutcome {
    Sent,
    /// No pane is bound, so nothing is listening.
    NotRunning,
}
