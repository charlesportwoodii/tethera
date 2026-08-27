use crate::protocol::terminal::TerminalFrame;
use crate::structs::ids::PaneId;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// One terminal frame, addressed.
///
/// The pane is carried rather than encoded into the event channel's name. A
/// phone can hold one pane's screen while a frame arrives for another, and a
/// frame that did not say which pane it belonged to would be applied to whatever
/// happened to be open - which on a terminal is invisible, because the wrong
/// output still looks like output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct PaneFrame {
    pub pane: PaneId,
    pub frame: TerminalFrame,
}
