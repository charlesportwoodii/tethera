use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A pane's grid dimensions.
///
/// Observed, never requested. There is no resize operation in this protocol in
/// either direction: geometry is decided by the server when it creates a pane
/// and is stable for that pane's life.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct Size {
    pub cols: u16,
    pub rows: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}
