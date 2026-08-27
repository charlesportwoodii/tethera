use serde::Deserialize;

/// How far back a pane can be scrolled.
///
/// Not required by the schema, and `max_offset_from_bottom + viewport_rows` is
/// an **upper bound** on the lines a read will return, never an exact count: a
/// pane whose viewport is mostly blank reports a full viewport and answers with
/// the one line it holds. Nothing here may be treated as a length.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct PaneScrollInfo {
    pub offset_from_bottom: u64,
    pub max_offset_from_bottom: u64,
    pub viewport_rows: u64,
}

impl PaneScrollInfo {
    /// The most lines a read could return. An upper bound; see above.
    pub fn at_most(&self) -> u64 {
        self.max_offset_from_bottom.saturating_add(self.viewport_rows)
    }
}
