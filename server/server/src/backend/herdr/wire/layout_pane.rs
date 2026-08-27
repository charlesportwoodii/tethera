use super::Rect;
use serde::Deserialize;

/// One pane's place in its tab's arrangement.
#[derive(Debug, Clone, Deserialize)]
pub struct LayoutPane {
    pub pane_id: String,
    pub focused: bool,
    pub rect: Rect,
}
