use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Where a pane sits inside its tab, in terminal cells.
///
/// Cells rather than pixels because a cell is the only unit both ends agree
/// on: the desk's window can be any size, and a phone drawing the same layout
/// scales the whole tab to its own width. A ratio would lose which pane is one
/// column wider, which is exactly what tells two nearly-equal splits apart.
///
/// The origin is the tab's own, not the screen's, so a client normalises
/// against the union of the tab's rects rather than needing the window
/// geometry as well.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct PaneRect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl PaneRect {
    pub fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// The first column past this rect's right edge.
    ///
    /// Saturating rather than wrapping: a backend that reports a rect running
    /// off the end of the tab is describing something wrong, and an edge that
    /// wraps to zero would place the pane back at the left margin and draw a
    /// map that looks plausible.
    pub fn right(&self) -> u16 {
        self.x.saturating_add(self.width)
    }

    /// The first row past this rect's bottom edge.
    pub fn bottom(&self) -> u16 {
        self.y.saturating_add(self.height)
    }
}
