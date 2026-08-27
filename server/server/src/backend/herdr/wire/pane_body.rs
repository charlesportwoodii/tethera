use super::PaneInfo;
use serde::Deserialize;

/// The `pane_info` answer, which wraps its pane in a field.
///
/// The alias covers a create-shaped answer naming the same pane `root_pane`, so
/// one type reads both without a caller having to know which it got.
#[derive(Debug, Clone, Deserialize)]
pub struct PaneBody {
    #[serde(alias = "root_pane")]
    pub pane: PaneInfo,
}
