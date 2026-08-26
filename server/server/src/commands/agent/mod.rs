mod catalog;
mod spawn;
mod sub_command;

pub use sub_command::AgentSubCommand;

use crate::config::ApplicationConfig;
use std::sync::Arc;

#[derive(clap::Args, Debug, Clone)]
pub struct Config {
    #[clap(subcommand)]
    pub cmd: AgentSubCommand,
}

impl Config {
    pub async fn run(&self, config: Arc<ApplicationConfig>) -> anyhow::Result<()> {
        match &self.cmd {
            AgentSubCommand::Catalog(command) => command.run(config).await,
            AgentSubCommand::Spawn(command) => command.run(config).await,
        }
    }
}
