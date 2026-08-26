use clap::Parser;
use std::path::PathBuf;
use tethera_relay::config::RelayConfig;
use tethera_relay::server::RelayServer;

#[derive(clap::Subcommand, Debug, Clone)]
enum SubCommand {
    /// Run the relay
    Run {
        /// Path to the relay configuration file
        #[clap(long, value_name = "PATH", env = "TETHERA_RELAY_CONFIG")]
        config: PathBuf,
    },
}

#[derive(Debug, Parser, Clone)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    #[clap(subcommand)]
    cmd: SubCommand,
}

impl Cli {
    async fn run() {
        let cli = Self::parse();

        let result = match &cli.cmd {
            SubCommand::Run { config } => Self::serve(config).await,
        };

        if let Err(error) = result {
            eprintln!("{error:#}");
            std::process::exit(1);
        }
    }

    async fn serve(path: &PathBuf) -> anyhow::Result<()> {
        let config = RelayConfig::from_file(path)?;

        RelayServer::new(config).run().await
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("TETHERA_LOG")
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    Cli::run().await;
}
