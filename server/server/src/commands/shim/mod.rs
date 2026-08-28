use std::sync::Arc;

use crate::config::ApplicationConfig;
use crate::protocol::ports::TerminalSession;
use crate::terminal::{PaneRegistry, Shim, ShimLink, ShimRelay};
use tethera_common::protocol::terminal::{Key, Mods, TerminalFrame, TerminalInput};
use tethera_common::structs::ids::PaneId;

/// `tethera shim` — the pane-side half of a streamable herdr pane.
///
/// Meant to be a herdr `default_shell`, so every pane on the machine runs it,
/// including panes split by hand at the desk. That reach is the point and it is
/// also the hazard: this runs when the tethera server is stopped, when it is
/// half upgraded, and in panes opened for work that has nothing to do with
/// tethera.
///
/// So the contract of this command is that **it always ends up at a shell.**
/// Every failure it can survive is survived, and the one it cannot — no pty —
/// falls through to executing the shell in place. A pane must never be left
/// dead because the shim had an opinion.
#[derive(clap::Args, Debug, Clone)]
pub struct Config {
    /// The shell to wrap. Defaults to TETHERA_SHIM_SHELL, then SHELL, then the
    /// platform's own
    #[clap(long)]
    pub shell: Option<String>,

    /// Accept shims and render what they relay, instead of being one.
    ///
    /// Spike scaffolding. The server's own runtime is where this belongs, and
    /// this exists so the whole chain can be watched from one terminal before it
    /// is wired into a runtime that also needs a phone to observe.
    #[clap(long)]
    pub listen: bool,
}

impl Config {
    pub async fn run(&self, config: Arc<ApplicationConfig>) -> anyhow::Result<()> {
        if self.listen {
            return Self::listen(&config).await;
        }

        let shell = self.shell.clone().unwrap_or_else(Shim::default_shell);

        // A shim inside a shim would open a second pty for no gain and would
        // double every byte's trip through a console translation. The marker is
        // set on the shell below, so it is inherited by anything the shell
        // launches.
        if std::env::var(Shim::MARKER).is_ok() {
            return Self::exec(&shell);
        }

        let address = ShimLink::address(&config.data_dir);

        match Shim::run(&shell, Some(&address)) {
            Ok(code) => std::process::exit(code),
            Err(error) => {
                // Written to stderr rather than through tracing: there is no
                // subscriber here, and the pane is the only place a person will
                // ever see this.
                eprintln!("tethera: could not wrap this shell ({error:#}); running it directly");

                Self::exec(&shell)
            }
        }
    }

    /// Serves the shim channel and prints what each relayed pane draws.
    async fn listen(config: &ApplicationConfig) -> anyhow::Result<()> {
        let address = ShimLink::address(&config.data_dir);
        let registry = PaneRegistry::new_shared();
        let relay = ShimRelay::new_shared(registry.clone());

        println!("listening for shims on {address}");

        Self::accept(registry, relay, &address).await
    }

    #[cfg(windows)]
    async fn accept(
        registry: Arc<PaneRegistry>,
        relay: Arc<ShimRelay>,
        address: &str,
    ) -> anyhow::Result<()> {
        use tokio::net::windows::named_pipe::ServerOptions;

        loop {
            // The next instance is created before the current one is handed off,
            // which is how a named pipe server stays reachable. Creating it after
            // leaves a window where a dialling shim is refused.
            let server = ServerOptions::new().create(address)?;
            server.connect().await?;

            let registry = Arc::clone(&registry);
            let relay = Arc::clone(&relay);

            tokio::spawn(async move {
                match Arc::clone(&relay).serve(server).await {
                    Ok(Some(pane)) => Self::watch(registry, relay, pane),
                    Ok(None) => {}
                    Err(error) => println!("a shim was refused: {error:#}"),
                }
            });
        }
    }

    #[cfg(unix)]
    async fn accept(
        registry: Arc<PaneRegistry>,
        relay: Arc<ShimRelay>,
        address: &str,
    ) -> anyhow::Result<()> {
        use tokio::net::UnixListener;

        // A socket file outlives the process that made it, so a restart binds a
        // path that already exists and fails. Removed rather than reused: the
        // old file names a socket nothing is listening on.
        let _ = std::fs::remove_file(address);

        let listener = UnixListener::bind(address)?;

        loop {
            let (stream, _) = listener.accept().await?;
            let registry = Arc::clone(&registry);
            let relay = Arc::clone(&relay);

            tokio::spawn(async move {
                match Arc::clone(&relay).serve(stream).await {
                    Ok(Some(pane)) => Self::watch(registry, relay, pane),
                    Ok(None) => {}
                    Err(error) => println!("a shim was refused: {error:#}"),
                }
            });
        }
    }


    /// Reports what a relayed pane actually draws.
    ///
    /// Spike proof rather than product behaviour. Attaches the same way a phone
    /// does and prints the shape of every frame, so the two facts under test are
    /// visible without a client: that frames arrive at all, and that a relayed
    /// pane reports a cursor - which a sampled one deliberately does not.
    fn watch(registry: Arc<PaneRegistry>, relay: Arc<ShimRelay>, pane: PaneId) {
        tokio::spawn(async move {
            let Ok(mut session) = registry.attach(&pane) else {
                return;
            };

            // Types into the pane exactly as an attached phone does, so the
            // downlink is proven by the shell's own echo rather than by a log
            // line saying bytes were written.
            // Claims the pane at a phone-sized geometry, as an attach will.
            if let Ok(claim) = std::env::var("TETHERA_SHIM_PROBE_CLAIM") {
                let mut parts = claim.split('x');
                let cols = parts.next().and_then(|v| v.parse().ok()).unwrap_or(58);
                let rows = parts.next().and_then(|v| v.parse().ok()).unwrap_or(30);

                tokio::time::sleep(std::time::Duration::from_secs(8)).await;

                let taken = relay.claim(&pane, tethera_common::structs::terminal::Size { cols, rows });

                println!("[{}] claimed {cols}x{rows} -> {taken}", pane.as_str());
            }

            if let Ok(probe) = std::env::var("TETHERA_SHIM_PROBE_INPUT") {
                tokio::time::sleep(std::time::Duration::from_secs(4)).await;

                let _ = session.send_input(TerminalInput::Text(probe)).await;
                let _ = session
                    .send_input(TerminalInput::Key {
                        key: Key::Enter,
                        mods: Mods::NONE,
                    })
                    .await;

                println!("[{}] sent a probe line as a client would", pane.as_str());
            }

            while let Some(frame) = session.next_frame().await {
                let line = match &frame {
                    TerminalFrame::Snapshot { cols, rows, cursor, alt_screen, rows_data, .. } => {
                        format!(
                            "snapshot {cols}x{rows} alt={alt_screen} rows={} cursor={cursor:?}",
                            rows_data.len()
                        )
                    }
                    TerminalFrame::Damage { rows_data, cursor, .. } => {
                        format!("damage rows={} cursor={cursor:?}", rows_data.len())
                    }
                    TerminalFrame::Resized { cols, rows } => format!("resized {cols}x{rows}"),
                    TerminalFrame::Bell => "bell".to_string(),
                    TerminalFrame::Closed { reason } => format!("closed {reason:?}"),
                };

                println!("[{}] {line}", pane.as_str());
            }
        });
    }

    /// Becomes the shell.
    ///
    /// The fallback that makes the whole arrangement safe to install machine
    /// wide. Not `exec` on Windows, which has no equivalent, so the shell is
    /// spawned and waited on and this process stays as its parent — one idle
    /// process per pane, which is the cheaper mistake.
    fn exec(shell: &str) -> anyhow::Result<()> {
        let mut command = std::process::Command::new(shell);
        let status = command.status()?;

        std::process::exit(status.code().unwrap_or(0));
    }
}
