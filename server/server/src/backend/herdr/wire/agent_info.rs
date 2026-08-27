use serde::Deserialize;

/// A pane that has an agent, as herdr reports it.
///
/// The snapshot's `agents` array is the subset of `panes` that has an agent,
/// with the agent lifecycle fields added. Nothing here is needed to fill a
/// pane: a pane's own record already carries `agent`, `terminal_title` and
/// `agent_session`. The fields unique to this array are the ones below, and
/// they belong to the conversation surface rather than to the tree.
#[derive(Debug, Clone, Deserialize)]
pub struct AgentInfo {
    pub pane_id: String,
    #[serde(default)]
    pub interactive_ready: bool,
    #[serde(default)]
    pub launch_pending: bool,
    #[serde(default)]
    pub screen_detection_skipped: bool,
    #[serde(default)]
    pub state_change_seq: u64,
    #[serde(default)]
    pub name: Option<String>,
}
