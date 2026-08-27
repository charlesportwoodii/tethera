use super::{
    AgentCapabilities, AgentProfile, AgentSpawn, CommandTags, NoiseFilter, ScreenChrome,
    TranscriptSource,
};
use crate::structs::ids::ProfileId;
use crate::traits::AgentTrait;
use std::path::Path;

pub struct CodexAgent;

impl CodexAgent {
    pub const BINARY: &'static str = "codex";
}

impl AgentTrait for CodexAgent {
    fn binary(&self) -> &'static str {
        Self::BINARY
    }

    fn launch_command(&self, spawn: &AgentSpawn) -> Vec<String> {
        if let Some(session) = &spawn.resume {
            return self.resume_command(session);
        }

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

    fn profile(&self) -> AgentProfile {
        AgentProfile {
            id: ProfileId(Self::BINARY.to_string()),
            label: "Codex".to_string(),
            description: None,
            version: None,
            supports_resume: true,
            // Nobody has measured Codex's records, so this build cannot read
            // them. Claiming otherwise would put an empty conversation in front
            // of a person instead of the terminal that does work.
            provides_transcript: false,
        }
    }

    fn transcript_source(&self, _home: &Path, _cwd: &str, _session: &str) -> TranscriptSource {
        TranscriptSource::Absent
    }

    fn noise_filter(&self) -> &'static NoiseFilter {
        &NoiseFilter::EMPTY
    }

    /// Nobody has measured how this harness records a command, so nothing here
    /// pretends to know. Borrowing the other harness's tags would read these
    /// records through a grammar that was never theirs.
    fn command_tags(&self) -> Option<&'static CommandTags> {
        None
    }

    /// Nobody has measured what this harness draws, so its screens are not read
    /// and its pickers are not driven. A guess would answer the wrong option on
    /// somebody's behalf and report that it had worked.
    fn screen_chrome(&self) -> Option<&'static ScreenChrome> {
        None
    }

    fn file_push_tools(&self) -> &'static [&'static str] {
        &[]
    }

    fn question_tools(&self) -> &'static [&'static str] {
        &[]
    }

    fn diff_tools(&self) -> &'static [&'static str] {
        &[]
    }

    fn todo_tools(&self) -> &'static [&'static str] {
        &[]
    }
}
