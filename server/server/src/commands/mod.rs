use crate::config::{ApplicationConfig, TerminalKind};
use clap::Parser;
use std::sync::Arc;

pub mod agent;
pub mod device;
pub mod herdr;
pub mod pair;
pub mod server;
pub mod shim;

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
    /// Run a shell inside a pty and copy it both ways, so a pane this process
    /// does not own still has a readable byte stream
    Shim(shim::Config),
    /// Write herdr's own configuration, so its panes start under the shim
    Herdr(herdr::Config),
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
    /// The file name that means "you are a pane's shell, not a CLI".
    ///
    /// herdr runs `default_shell` with **no arguments** — measured, by pointing
    /// it at a script that logged its own argv and getting `ARGS:[]` — and it
    /// discards any arguments written into the setting itself, so
    /// `default_shell = "tethera.exe shim"` loses the subcommand.
    ///
    /// That leaves the binary's own name as the only channel. Invoked as
    /// `tethera-shim`, it is a shell; invoked as `tethera`, it is the CLI, and a
    /// bare `tethera` still prints its usage rather than silently becoming a
    /// terminal.
    ///
    /// One build either way: the hook links or copies the installed binary to
    /// this name rather than shipping a second one.
    pub const SHIM_ARGV0: &'static str = crate::terminal::Shim::ARGV0;

    /// Whether this process was invoked under the shim's name.
    ///
    /// Checked before `Cli::parse`, because a shell receives arguments that are
    /// not this CLI's and clap would exit on them. A bare `tethera.exe` with no
    /// subcommand is a usage error exiting 2 — which, as `default_shell`, killed
    /// every new tab, split and agent start on the machine at once.
    pub fn invoked_as_shim() -> bool {
        std::env::args_os()
            .next()
            .map(std::path::PathBuf::from)
            .and_then(|path| {
                path.file_stem()
                    .map(|stem| stem.to_string_lossy().to_ascii_lowercase())
            })
            .is_some_and(|stem| stem == Self::SHIM_ARGV0)
    }

    /// Runs as a pane's shell, taking every argument as the shell's.
    ///
    /// Never parses with clap. Whatever a terminal manager hands its shell
    /// belongs to the shell.
    pub async fn run_as_shim() {
        let config = Arc::new(ApplicationConfig::default());
        let args: Vec<String> = std::env::args().skip(1).collect();
        let command = shim::Config {
            shell: None,
            args,
        };

        if let Err(error) = command.run(config).await {
            eprintln!("{error:#}");
        }
    }

    /// Whether this invocation is a pane's shell rather than a command.
    ///
    /// The shim must reach a shell even when nothing else on this machine works,
    /// so it is dispatched before the data-directory check that every other
    /// subcommand genuinely needs. Once herdr's `default_shell` names this
    /// binary, a full disk would otherwise leave the operator with panes that
    /// have no shell at all.
    pub fn is_shim(&self) -> bool {
        matches!(self.cmd, SubCommand::Shim(_))
    }

    pub async fn run() {
        let cli = Self::parse();
        let config = Arc::new(cli.application_config());

        // Before `ensure_data_dir`, deliberately. `ShimLink::address` only builds
        // a path, so a missing directory costs a failed dial and the shim runs
        // the shell - which is the whole contract.
        if let SubCommand::Shim(command) = &cli.cmd {
            if let Err(error) = command.run(config).await {
                eprintln!("{error:#}");
            }

            return;
        }

        if let Err(error) = config.ensure_data_dir() {
            eprintln!("cannot create data directory: {error}");
            std::process::exit(1);
        }

        let result = match &cli.cmd {
            SubCommand::Server(command) => command.run(config).await,
            SubCommand::Device(command) => command.run(config).await,
            SubCommand::Pair(command) => command.run(config).await,
            SubCommand::Agent(command) => command.run(config).await,
            SubCommand::Shim(command) => command.run(config).await,
            SubCommand::Herdr(command) => command.run(config).await,
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
