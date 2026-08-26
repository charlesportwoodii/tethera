use crate::config::ApplicationConfig;
use crate::identity::Identity;
use std::sync::Arc;

#[derive(clap::Args, Debug, Clone)]
pub struct Config {}

impl Config {
    pub async fn run(&self, config: Arc<ApplicationConfig>) -> anyhow::Result<()> {
        match Identity::load_or_report_absent(&config.identity_path())? {
            Some(secret_key) => println!("endpoint id: {}", secret_key.public()),
            None => println!("endpoint id: none yet; the first server start creates it"),
        }

        println!("data dir:    {}", config.data_dir.display());

        match Self::read_pid(&config)? {
            Some(pid) => println!("running:     yes, pid {pid}"),
            None => println!("running:     no"),
        }

        Ok(())
    }

    fn read_pid(config: &ApplicationConfig) -> anyhow::Result<Option<u32>> {
        let path = config.pid_path();

        if !path.exists() {
            return Ok(None);
        }

        Ok(std::fs::read_to_string(path)?.trim().parse::<u32>().ok())
    }
}
