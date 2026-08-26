use crate::config::ApplicationConfig;
use std::sync::Arc;

#[derive(clap::Args, Debug, Clone)]
pub struct Config {}

impl Config {
    pub async fn run(&self, config: Arc<ApplicationConfig>) -> anyhow::Result<()> {
        println!("will list devices from {}", config.database_path().display());

        Ok(())
    }
}
