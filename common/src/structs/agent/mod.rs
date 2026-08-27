mod capabilities;
mod chrome;
mod claude;
mod codex;
mod commands;
mod noise;
mod profile;
mod source;
mod spawn;
mod status;

pub use capabilities::AgentCapabilities;
pub use chrome::ScreenChrome;
pub use claude::ClaudeAgent;
pub use codex::CodexAgent;
pub use commands::CommandTags;
pub use noise::NoiseFilter;
pub use profile::AgentProfile;
pub use source::TranscriptSource;
pub use spawn::AgentSpawn;
pub use status::AgentStatus;

use crate::traits::AgentTrait;
use serde::{Deserialize, Serialize};
use std::path::Path;

// No `TS` derive: the agent's identity never crosses the wire. The client sees
// `AgentProfile`, so adding an agent is a trait implementation and a catalog row
// rather than a client release. `clap::ValueEnum` stays - a closed set is right
// for an argument a person types at the machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Agent {
    Claude,
    Codex,
}

impl Agent {
    pub const ALL: [Agent; 2] = [Agent::Claude, Agent::Codex];
}

impl AgentTrait for Agent {
    fn binary(&self) -> &'static str {
        match self {
            Self::Claude => ClaudeAgent.binary(),
            Self::Codex => CodexAgent.binary(),
        }
    }

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

    fn profile(&self) -> AgentProfile {
        match self {
            Self::Claude => ClaudeAgent.profile(),
            Self::Codex => CodexAgent.profile(),
        }
    }

    fn transcript_source(&self, home: &Path, cwd: &str, session: &str) -> TranscriptSource {
        match self {
            Self::Claude => ClaudeAgent.transcript_source(home, cwd, session),
            Self::Codex => CodexAgent.transcript_source(home, cwd, session),
        }
    }

    fn noise_filter(&self) -> &'static NoiseFilter {
        match self {
            Self::Claude => ClaudeAgent.noise_filter(),
            Self::Codex => CodexAgent.noise_filter(),
        }
    }

    fn command_tags(&self) -> Option<&'static CommandTags> {
        match self {
            Self::Claude => ClaudeAgent.command_tags(),
            Self::Codex => CodexAgent.command_tags(),
        }
    }

    fn screen_chrome(&self) -> Option<&'static ScreenChrome> {
        match self {
            Self::Claude => ClaudeAgent.screen_chrome(),
            Self::Codex => CodexAgent.screen_chrome(),
        }
    }

    fn file_push_tools(&self) -> &'static [&'static str] {
        match self {
            Self::Claude => ClaudeAgent.file_push_tools(),
            Self::Codex => CodexAgent.file_push_tools(),
        }
    }

    fn question_tools(&self) -> &'static [&'static str] {
        match self {
            Self::Claude => ClaudeAgent.question_tools(),
            Self::Codex => CodexAgent.question_tools(),
        }
    }

    fn diff_tools(&self) -> &'static [&'static str] {
        match self {
            Self::Claude => ClaudeAgent.diff_tools(),
            Self::Codex => CodexAgent.diff_tools(),
        }
    }

    fn todo_tools(&self) -> &'static [&'static str] {
        match self {
            Self::Claude => ClaudeAgent.todo_tools(),
            Self::Codex => CodexAgent.todo_tools(),
        }
    }
}
