use super::WorktreeInfo;
use serde::Deserialize;

/// One workspace, as herdr reports it.
///
/// `agent_status` is a `String` rather than an enum: nothing in this mapping
/// reads it, and a herdr release that adds a status must not fail a listing in
/// a backend that never looked.
#[derive(Debug, Clone, Deserialize)]
pub struct WorkspaceInfo {
    pub workspace_id: String,
    pub number: u64,
    pub label: String,
    pub focused: bool,
    pub pane_count: u64,
    pub tab_count: u64,
    pub active_tab_id: String,
    #[serde(default)]
    pub agent_status: Option<String>,
    #[serde(default)]
    pub worktree: Option<WorktreeInfo>,
}
