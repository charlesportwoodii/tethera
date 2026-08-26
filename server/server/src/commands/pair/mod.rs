mod code;
mod qr;
mod sub_command;

pub use sub_command::PairSubCommand;

use crate::config::ApplicationConfig;
use std::sync::Arc;

#[derive(clap::Args, Debug, Clone)]
pub struct Config {
    #[clap(subcommand)]
    pub cmd: PairSubCommand,
}

impl Config {
    pub async fn run(&self, config: Arc<ApplicationConfig>) -> anyhow::Result<()> {
        match &self.cmd {
            PairSubCommand::Qr(command) => command.run(config).await,
            PairSubCommand::Code(command) => command.run(config).await,
        }
    }
}
