use super::{catalog, spawn};

#[derive(clap::Subcommand, Debug, Clone)]
pub enum AgentSubCommand {
    /// List the agents this host accepts
    Catalog(catalog::Config),
    /// Start an agent in a new herdr workspace
    Spawn(spawn::Config),
}
