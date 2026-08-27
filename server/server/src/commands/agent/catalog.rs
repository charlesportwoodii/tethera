use crate::config::ApplicationConfig;
use crate::machine::Installed;
use std::sync::Arc;
use tethera_common::structs::agent::Agent;
use tethera_common::traits::AgentTrait;

#[derive(clap::Args, Debug, Clone)]
pub struct Config {}

impl Config {
    // println! is correct here. stdout is this subcommand's output medium,
    // not a logging channel.
    pub async fn run(&self, _config: Arc<ApplicationConfig>) -> anyhow::Result<()> {
        for agent in Agent::ALL {
            let profile = agent.profile();

            // Every agent this build can drive, with whether the machine
            // actually has it. A client is sent only the installed ones, and an
            // operator asking why a harness is missing from their phone needs to
            // see the row that is not being sent.
            println!(
                "{}\t{}\tinstalled={}\tversion={}\tresume={}\ttranscript={}",
                profile.id.as_str(),
                profile.label,
                Installed::has(agent.binary()),
                profile.version.as_deref().unwrap_or("unknown"),
                profile.supports_resume,
                profile.provides_transcript
            );
        }

        Ok(())
    }
}
