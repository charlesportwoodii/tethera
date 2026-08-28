mod process;
pub mod start;
mod status;
mod stop;
mod sub_command;

pub use sub_command::ServerSubCommand;

use crate::config::ApplicationConfig;
use std::sync::Arc;

#[derive(clap::Args, Debug, Clone)]
pub struct Config {
    #[clap(subcommand)]
    pub cmd: ServerSubCommand,
}

impl Config {
    pub async fn run(&self, config: Arc<ApplicationConfig>) -> anyhow::Result<()> {
        match &self.cmd {
            ServerSubCommand::Start(command) => command.run(config).await,
            ServerSubCommand::Stop(command) => command.run(config).await,
            ServerSubCommand::Status(command) => command.run(config).await,
        }
    }
}
