mod code;
mod open;
mod qr;
mod sub_command;

pub use sub_command::PairSubCommand;

use crate::config::ApplicationConfig;
use std::sync::Arc;

#[derive(clap::Args, Debug, Clone)]
pub struct Config {
    // Flattened rather than held as a subcommand's arguments, so
    // `tethera pair --ttl-seconds 600` reaches the code that honours it. Behind
    // a subcommand these options would parse nowhere and read as accepted.
    #[clap(flatten)]
    pub open: open::Config,

    // Absent is the whole point: `tethera pair` on its own is what a person
    // runs, and it is the command three mockups print on the pairing screen.
    // The subcommands are the halves of it, for scripting.
    #[clap(subcommand)]
    pub cmd: Option<PairSubCommand>,
}

impl Config {
    pub async fn run(&self, config: Arc<ApplicationConfig>) -> anyhow::Result<()> {
        match &self.cmd {
            None => self.open.run(config).await,
            Some(PairSubCommand::Qr(command)) => command.run(config).await,
            Some(PairSubCommand::Code(command)) => command.run(config).await,
        }
    }
}
