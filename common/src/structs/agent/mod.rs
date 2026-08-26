mod capabilities;
mod claude;
mod codex;
mod spawn;
mod status;

pub use capabilities::AgentCapabilities;
pub use claude::ClaudeAgent;
pub use codex::CodexAgent;
pub use spawn::AgentSpawn;
pub use status::AgentStatus;

use crate::structs::transcript::TranscriptEntry;
use crate::traits::AgentTrait;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS, clap::ValueEnum,
)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub enum Agent {
    Claude,
    Codex,
}

impl Agent {
    pub const ALL: [Agent; 2] = [Agent::Claude, Agent::Codex];
}

impl AgentTrait for Agent {
    fn launch_command(&self, spawn: &AgentSpawn) -> Vec<String> {
        match self {
            Self::Claude => ClaudeAgent.launch_command(spawn),
            Self::Codex => CodexAgent.launch_command(spawn),
        }
    }

    fn resume_command(&self, session_id: &str) -> Vec<String> {
        match self {
            Self::Claude => ClaudeAgent.resume_command(session_id),
            Self::Codex => CodexAgent.resume_command(session_id),
        }
    }

    fn capabilities(&self) -> AgentCapabilities {
        match self {
            Self::Claude => ClaudeAgent.capabilities(),
            Self::Codex => CodexAgent.capabilities(),
        }
    }

    fn parse_transcript(&self, raw: &str) -> Vec<TranscriptEntry> {
        match self {
            Self::Claude => ClaudeAgent.parse_transcript(raw),
            Self::Codex => CodexAgent.parse_transcript(raw),
        }
    }
}
