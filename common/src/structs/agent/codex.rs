use super::{AgentCapabilities, AgentSpawn};
use crate::structs::transcript::TranscriptEntry;
use crate::traits::AgentTrait;

pub struct CodexAgent;

impl CodexAgent {
    pub const BINARY: &'static str = "codex";
}

impl AgentTrait for CodexAgent {
    fn launch_command(&self, spawn: &AgentSpawn) -> Vec<String> {
        let mut argv = vec![Self::BINARY.to_string()];

        if let Some(prompt) = &spawn.prompt {
            argv.push(prompt.clone());
        }

        argv
    }

    fn resume_command(&self, session_id: &str) -> Vec<String> {
        vec![
            Self::BINARY.to_string(),
            "--resume".to_string(),
            session_id.to_string(),
        ]
    }

    fn capabilities(&self) -> AgentCapabilities {
        AgentCapabilities {
            resume: true,
            interrupt: true,
            file_upload: false,
            questions: false,
        }
    }

    // The transcript format is owned by the protocol agents. Until they define
    // it, an unparsed transcript is one Unknown part carrying the source rows,
    // which is exactly the contract a client already handles.
    fn parse_transcript(&self, raw: &str) -> Vec<TranscriptEntry> {
        TranscriptEntry::unparsed(Self::BINARY, raw)
    }
}
