mod approve;
mod ban;
mod list;
mod revoke;
mod sub_command;
mod unban;

pub use sub_command::DeviceSubCommand;

use crate::config::ApplicationConfig;
use std::sync::Arc;

#[derive(clap::Args, Debug, Clone)]
pub struct Config {
    #[clap(subcommand)]
    pub cmd: DeviceSubCommand,
}

impl Config {
    pub async fn run(&self, config: Arc<ApplicationConfig>) -> anyhow::Result<()> {
        match &self.cmd {
            DeviceSubCommand::List(command) => command.run(config).await,
            DeviceSubCommand::Approve(command) => command.run(config).await,
            DeviceSubCommand::Revoke(command) => command.run(config).await,
            DeviceSubCommand::Ban(command) => command.run(config).await,
            DeviceSubCommand::Unban(command) => command.run(config).await,
        }
    }
}
