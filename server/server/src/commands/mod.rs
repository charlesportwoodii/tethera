use crate::config::ApplicationConfig;
use clap::Parser;
use std::sync::Arc;

pub mod agent;
pub mod device;
pub mod pair;
pub mod server;

#[derive(clap::Subcommand, Debug, Clone)]
pub enum SubCommand {
    /// Run and control the Tethera server
    Server(server::Config),
    /// Manage paired devices
    Device(device::Config),
    /// Pair a new device
    Pair(pair::Config),
    /// Inspect and start agents
    Agent(agent::Config),
}

#[derive(Debug, Parser, Clone)]
#[clap(author, version, about, long_about = None)]
pub struct Cli {
    /// Override the platform data directory
    #[clap(global = true, long, env = "TETHERA_DATA_DIR")]
    pub data_dir: Option<std::path::PathBuf>,

    #[clap(subcommand)]
    pub cmd: SubCommand,
}

impl Cli {
    pub async fn run() {
        let cli = Self::parse();
        let config = cli.application_config();

        if let Err(error) = config.ensure_data_dir() {
            eprintln!("cannot create data directory: {error}");
            std::process::exit(1);
        }

        let config = Arc::new(config);

        let result = match &cli.cmd {
            SubCommand::Server(command) => command.run(config).await,
            SubCommand::Device(command) => command.run(config).await,
            SubCommand::Pair(command) => command.run(config).await,
            SubCommand::Agent(command) => command.run(config).await,
        };

        if let Err(error) = result {
            eprintln!("{error:#}");
            std::process::exit(1);
        }
    }

    fn application_config(&self) -> ApplicationConfig {
        match &self.data_dir {
            Some(dir) => ApplicationConfig::with_data_dir(dir.clone()),
            None => ApplicationConfig::default(),
        }
    }
}
