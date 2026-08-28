use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// What a machine will let a terminal screen do.
///
/// Read from the capability set recorded at the last handshake rather than
/// discovered by trying. A control drawn and then refused on press teaches
/// somebody that the app is unreliable; a control that is absent, with a line
/// saying why, teaches them what this machine is.
///
/// Booleans rather than the raw set, because the screen asks the same questions
/// every time and a set would put the capability names in the TypeScript, where
/// a rename goes unnoticed until a control quietly stops working.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct TerminalControls {
    pub attach: bool,
    pub input: bool,
    pub scrollback: bool,
    /// Whether a pane's frames are the program's own bytes.
    ///
    /// False means the screen is sampled on a timer, so a full-screen program
    /// piles up instead of redrawing and there is no cursor to draw. The screen
    /// says so once, in a line beside the pane, rather than leaving somebody to
    /// conclude the app is broken.
    pub streamed: bool,
    pub open: bool,
    pub split: bool,
    pub close: bool,
    /// Whether this machine will say where a tab's panes sit.
    ///
    /// False means the floorplan is absent rather than empty. `Floorplan.place`
    /// returns nothing without a layout, and `WorkspaceMap` draws nothing for
    /// nothing — an empty bordered rectangle reads as a workspace with no panes
    /// in it, which is a different and wrong statement.
    pub layout: bool,
    /// Whether tapping a tab here also moves the desk to it.
    pub focus_tab: bool,
    /// Whether this machine can return output with its wrapping removed.
    ///
    /// False means the view toggle is absent rather than present and handing
    /// back the other view. A pty publishes bytes already laid out for its own
    /// width, and un-wrapping them would need the emulator to record which line
    /// breaks were autowrap; nothing does that yet.
    pub lines_view: bool,
}
