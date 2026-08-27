use serde::Deserialize;

/// Where a workspace's git worktree is checked out.
///
/// herdr records no working directory on a workspace itself. This is the only
/// place one exists, which is why `Workspace.cwd` prefers it.
#[derive(Debug, Clone, Deserialize)]
pub struct WorktreeInfo {
    pub repo_key: String,
    pub repo_name: String,
    pub repo_root: String,
    pub checkout_path: String,
    pub is_linked_worktree: bool,
}
