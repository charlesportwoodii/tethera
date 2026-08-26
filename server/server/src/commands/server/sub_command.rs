use super::{start, status, stop};

#[derive(clap::Subcommand, Debug, Clone)]
pub enum ServerSubCommand {
    /// Start the Tethera server
    Start(start::Config),
    /// Stop a running Tethera server
    Stop(stop::Config),
    /// Report whether a Tethera server is running
    Status(status::Config),
}
