use crate::config::{ApplicationConfig, TerminalKind};
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

    /// What a pairing screen calls this machine. Defaults to its hostname
    #[clap(global = true, long, env = "TETHERA_LABEL")]
    pub label: Option<String>,

    /// The relay to reach this machine through. Its shared secret is read from
    /// TETHERA_RELAY_TOKEN, which has no flag on purpose
    #[clap(global = true, long, env = "TETHERA_RELAY_URL")]
    pub relay_url: Option<String>,

    /// Which terminal backend to drive. Only `pty` can be attached to: herdr
    /// publishes no per-pane byte stream
    #[clap(global = true, long, value_enum, default_value_t, env = "TETHERA_TERMINAL")]
    pub terminal_backend: TerminalKind,

    /// The UDP port this machine's endpoint binds. Forward it at the router to
    /// let a phone outside the network reach this machine directly rather than
    /// through a relay
    #[clap(global = true, long, default_value_t = ApplicationConfig::DEFAULT_BIND_PORT, env = "TETHERA_BIND_PORT")]
    pub bind_port: u16,

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

    pub const RELAY_TOKEN_ENV: &'static str = "TETHERA_RELAY_TOKEN";

    // The relay URL reaches a QR, stdout, `endpoint.json` and the log, so a
    // credential inside it would be published four ways. Stripped once here
    // rather than at each of those, because none of them can tell the
    // difference and each would have to remember.
    fn relay_url_without_secrets(raw: &str) -> String {
        let (cleaned, removed) = ApplicationConfig::sanitise_relay_url(raw);

        if removed {
            eprintln!(
                "the relay url carried a query string or credentials; they were removed. \
                 the relay's shared secret belongs in TETHERA_RELAY_TOKEN, never in the url, \
                 because the url is published in the pairing offer"
            );
        }

        cleaned
    }

    fn application_config(&self) -> ApplicationConfig {
        let mut config = match &self.data_dir {
            Some(dir) => ApplicationConfig::with_data_dir(dir.clone()),
            None => ApplicationConfig::default(),
        };

        config.label = self.label.clone();
        config.terminal_backend = self.terminal_backend;
        config.bind_port = self.bind_port;
        config.relay_url = self.relay_url.as_deref().map(Self::relay_url_without_secrets);
        // Read from the environment and given no flag. A secret handed to argv
        // is readable by anything that can call ps and is written to the shell
        // history verbatim, which is the same reason `device approve` takes its
        // code on stdin.
        config.relay_token = std::env::var(Self::RELAY_TOKEN_ENV).ok().filter(|token| !token.is_empty());

        config
    }
}
