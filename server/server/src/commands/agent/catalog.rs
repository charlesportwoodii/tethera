use crate::config::ApplicationConfig;
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
            let caps = agent.capabilities();
            println!(
                "{agent:?}\tresume={}\tinterrupt={}\tfile_upload={}\tquestions={}",
                caps.resume, caps.interrupt, caps.file_upload, caps.questions
            );
        }

        Ok(())
    }
}
