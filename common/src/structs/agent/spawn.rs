use super::Agent;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct AgentSpawn {
    pub agent: Agent,
    pub cwd: String,
    pub prompt: Option<String>,
}

impl AgentSpawn {
    pub fn new(agent: Agent, cwd: String, prompt: Option<String>) -> Self {
        Self { agent, cwd, prompt }
    }
}
