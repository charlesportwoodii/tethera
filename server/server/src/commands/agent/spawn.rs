use crate::config::ApplicationConfig;
use crate::protocol::live::{LiveConversations, LiveTerminals};
use crate::protocol::ports::ConversationPort;
use crate::transcript::AssetIndex;
use std::sync::Arc;
use tethera_common::structs::agent::Agent;
use tethera_common::traits::agent::AgentTrait;

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
    pub async fn run(&self, config: Arc<ApplicationConfig>) -> anyhow::Result<()> {
        // The same call the phone makes, over the same port. A second
        // implementation here would be a second set of rules about where an
        // agent may start and what happens to the pane when it will not, and the
        // two would drift.
        //
        // No database and no endpoint: starting an agent is terminal work, and
        // nothing about it is persisted or served. This runs against a machine
        // whether or not a server is up.
        let terminals = LiveTerminals::from_config(&config);
        let conversations = LiveConversations::new_shared(
            terminals,
            AssetIndex::new_shared(),
            config.data_dir.join("uploads"),
        );

        let started = conversations
            .start(
                &self.agent.profile().id,
                &self.cwd,
                self.prompt.as_deref(),
                &[],
            )
            .await
            .map_err(|error| {
                anyhow::anyhow!(
                    "could not start {} in {}: {error:?}",
                    self.agent.profile().label,
                    self.cwd
                )
            })?;

        println!("{}", started.id.as_str());

        if let Some(pane) = &started.binding {
            println!("pane {}", pane.as_str());
        }

        if let Some(workspace) = &started.workspace {
            println!("workspace {}", workspace.as_str());
        }

        Ok(())
    }
}
