use crate::config::ApplicationConfig;
use std::sync::Arc;

#[derive(clap::Args, Debug, Clone)]
pub struct Config {
    /// The device to ban
    pub id: String,
}

impl Config {
    pub async fn run(&self, _config: Arc<ApplicationConfig>) -> anyhow::Result<()> {
        println!("will ban device {}", self.id);

        Ok(())
    }
}
