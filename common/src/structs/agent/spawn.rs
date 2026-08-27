use super::Agent;
use serde::{Deserialize, Serialize};

// No `TS` derive, for the same reason `Agent` has none: this names the enum, and
// the enum is how the machine's own CLI is typed. A client starts a conversation
// with `StartConversation { profile, cwd, .. }`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSpawn {
    pub agent: Agent,
    pub cwd: String,
    pub prompt: Option<String>,
    /// The agent's own session to pick up, rather than beginning a new one.
    ///
    /// Carried on the spawn rather than expressed as a second kind of launch,
    /// because everything below this is identical either way: the same pane, the
    /// same readiness wait, the same shell the line is typed at. Only the argv
    /// differs, and which argv an agent takes is already a table on `AgentTrait`.
    pub resume: Option<String>,
}

impl AgentSpawn {
    pub fn new(agent: Agent, cwd: String, prompt: Option<String>) -> Self {
        Self {
            agent,
            cwd,
            prompt,
            resume: None,
        }
    }

    /// The same spawn, picking up a session this agent already has records for.
    ///
    /// No prompt: a resume puts a person back where they were, and a prompt
    /// delivered in the same breath would speak before they had read it.
    pub fn resuming(agent: Agent, cwd: String, session: String) -> Self {
        Self {
            agent,
            cwd,
            prompt: None,
            resume: Some(session),
        }
    }
}
