use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Which reading of a pane an attach wants.
///
/// Not a rendering preference. The two differ in what the backend is asked for
/// and therefore in what the emulator is fed, so a client that switches is asking
/// for a different stream rather than a different stylesheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub enum PaneView {
    /// Output as logical lines, laid out at the client's own width.
    ///
    /// The only view that never scrolls sideways, and the only one a phone can
    /// read comfortably against a pane laid out for a desk. It needs a backend
    /// that can return output with its wrapping removed, so it is not offered
    /// everywhere - see `capability::TERMINAL_LINES_VIEW`.
    Lines,
    /// The pane exactly as it stands, at its own width.
    ///
    /// Faithful, and wider than a phone. A client scrolls it.
    Screen,
}

impl Default for PaneView {
    /// `Lines`, because a pane laid out for a desk is unreadable on a phone any
    /// other way, and a phone is what this protocol exists to serve.
    fn default() -> Self {
        Self::Lines
    }
}
