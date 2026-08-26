use crate::config::ApplicationConfig;
use std::sync::Arc;
use tethera_common::structs::agent::Agent;

#[derive(clap::Args, Debug, Clone)]
pub struct Config {
    /// Which agent to start
    #[clap(long, value_enum)]
    pub agent: Agent,

    /// The working directory the agent runs in
    #[clap(long)]
    pub cwd: String,

    /// An opening prompt to hand the agent
    #[clap(long)]
    pub prompt: Option<String>,
}

impl Config {
    pub async fn run(&self, _config: Arc<ApplicationConfig>) -> anyhow::Result<()> {
        println!("will spawn {:?} in {}", self.agent, self.cwd);

        Ok(())
    }
}
