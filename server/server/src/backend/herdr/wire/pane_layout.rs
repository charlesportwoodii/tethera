use super::{LayoutPane, LayoutSplit, Rect};
use serde::Deserialize;

/// One tab's pane arrangement.
///
/// The only place herdr reports geometry, and it is live: splitting a pane
/// re-lays-out its neighbours, so a rect observed here is what the pane is now
/// rather than what it was created as. `panes` is in layout order, which is the
/// order a person reading the screen would call first-to-last.
#[derive(Debug, Clone, Deserialize)]
pub struct PaneLayout {
    pub workspace_id: String,
    pub tab_id: String,
    pub zoomed: bool,
    pub area: Rect,
    pub focused_pane_id: String,
    pub panes: Vec<LayoutPane>,
    #[serde(default)]
    pub splits: Vec<LayoutSplit>,
}

impl PaneLayout {
    pub fn rect_of(&self, pane_id: &str) -> Option<Rect> {
        self.panes
            .iter()
            .find(|pane| pane.pane_id == pane_id)
            .map(|pane| pane.rect)
    }
}
