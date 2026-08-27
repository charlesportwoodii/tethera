use super::{AgentSession, PaneScrollInfo};
use serde::Deserialize;

/// One pane, as herdr reports it.
///
/// `api snapshot` and `pane list` return byte-identical records: a pane running
/// an agent carries `agent`, `terminal_title` and `agent_session` inline, so
/// nothing has to be joined from the snapshot's `agents` array to fill a pane.
/// Every field past the first six is optional in herdr's own schema and is
/// therefore optional here, whatever a given capture happens to contain.
#[derive(Debug, Clone, Deserialize)]
pub struct PaneInfo {
    pub pane_id: String,
    pub terminal_id: String,
    pub workspace_id: String,
    pub tab_id: String,
    pub focused: bool,
    pub revision: u64,
    #[serde(default)]
    pub agent_status: Option<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub foreground_cwd: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub terminal_title: Option<String>,
    #[serde(default)]
    pub terminal_title_stripped: Option<String>,
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub display_agent: Option<String>,
    #[serde(default)]
    pub agent_session: Option<AgentSession>,
    #[serde(default)]
    pub scroll: Option<PaneScrollInfo>,
}
