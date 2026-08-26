use crate::structs::agent::{AgentCapabilities, AgentSpawn};
use crate::structs::transcript::TranscriptEntry;

pub trait AgentTrait {
    fn launch_command(&self, spawn: &AgentSpawn) -> Vec<String>;

    fn resume_command(&self, session_id: &str) -> Vec<String>;

    fn capabilities(&self) -> AgentCapabilities;

    fn parse_transcript(&self, raw: &str) -> Vec<TranscriptEntry>;
}
