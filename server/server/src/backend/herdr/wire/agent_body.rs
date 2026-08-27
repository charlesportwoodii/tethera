use super::PaneInfo;
use serde::Deserialize;

/// The `agent_started`, `agent_info` and `agent_prompted` answers, which all
/// wrap one agent in the same field.
///
/// `PaneInfo` and not `AgentInfo`: what a caller needs back from a start is the
/// session the agent announced, and that lives on the pane's own record.
/// `AgentInfo` carries the lifecycle fields beside it and none of the identity.
#[derive(Debug, Clone, Deserialize)]
pub struct AgentBody {
    pub agent: PaneInfo,
}
