use super::{PaneInfo, TabInfo, WorkspaceInfo};
use serde::Deserialize;

/// What `workspace create` and `tab create` answer with.
///
/// One type for both: `workspace_created` carries a workspace and
/// `tab_created` does not, and everything else is identical. A create naming
/// what it made is why nothing here creates and then lists to find it.
#[derive(Debug, Clone, Deserialize)]
pub struct Created {
    #[serde(default)]
    pub workspace: Option<WorkspaceInfo>,
    pub tab: TabInfo,
    pub root_pane: PaneInfo,
}
