use serde::Deserialize;

/// An agent's own session identity, as the agent reported it to herdr.
///
/// Populated only when something calls `herdr pane report-agent-session`. herdr
/// does not discover it, so it is absent on a pane whose agent never announced
/// itself.
#[derive(Debug, Clone, Deserialize)]
pub struct AgentSession {
    pub source: String,
    pub agent: String,
    pub kind: AgentSessionKind,
    pub value: String,
}

/// `Unknown` exists so a herdr release that adds a kind cannot fail a whole
/// snapshot parse over one pane. It maps to no conversation, which is the same
/// answer as an absent session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentSessionKind {
    Id,
    Path,
    #[serde(other)]
    Unknown,
}
