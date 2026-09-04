use std::sync::Arc;

use crate::config::ApplicationConfig;
use crate::terminal::{Shim, ShimLink};

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
///
/// The server side of the channel is not here. `ShimListener` runs inside the
/// server's own runtime, beside the registry a shim adopts into.
#[derive(clap::Args, Debug, Clone)]
pub struct Config {
    /// The shell to wrap. Defaults to TETHERA_SHIM_SHELL, then SHELL, then the
    /// platform's own
    #[clap(long)]
    pub shell: Option<String>,

    /// Everything else, passed to the shell untouched.
    ///
    /// herdr does not exec a `default_shell` bare — it invokes it the way it
    /// invokes a shell, as `<shell> -NoExit -Command "<prompt integration>"`.
    /// So this has to swallow arguments it knows nothing about and hand them
    /// on, hyphens and all.
    ///
    /// Rejecting them is not a degraded pane, it is no pane: clap exited before
    /// the shell started and every new tab, split and agent start on the machine
    /// failed at once.
    #[clap(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

impl Config {
    pub async fn run(&self, config: Arc<ApplicationConfig>) -> anyhow::Result<()> {
        let shell = self.shell.clone().unwrap_or_else(Shim::default_shell);

        // A shim inside a shim would open a second pty for no gain and would
        // double every byte's trip through a console translation. The marker is
        // set on the shell below, so it is inherited by anything the shell
        // launches.
        if std::env::var(Shim::MARKER).is_ok() {
            return Self::exec(&shell, &self.args);
        }

        let address = ShimLink::address(&config.data_dir);

        match Shim::run(&shell, &self.args, Some(&address)) {
            Ok(code) => std::process::exit(code),
            Err(error) => {
                // Written to stderr rather than through tracing: there is no
                // subscriber here, and the pane is the only place a person will
                // ever see this.
                eprintln!("tethera: could not wrap this shell ({error:#}); running it directly");

                Self::exec(&shell, &self.args)
            }
        }
    }

    /// Becomes the shell.
    ///
    /// The fallback that makes the whole arrangement safe to install machine
    /// wide: every path that cannot wrap the shell ends here instead of ending
    /// the pane.
    ///
    /// Replaces this process rather than parenting the shell. A process per pane
    /// is a cost nobody can see and nobody will attribute, and once
    /// `default_shell` names this binary there is one for every pane on the
    /// machine.
    #[cfg(unix)]
    fn exec(shell: &str, args: &[String]) -> anyhow::Result<()> {
        use std::os::unix::process::CommandExt;

        // `exec` only returns on failure, and then the shell genuinely could not
        // start — there is nothing left to fall back to.
        Err(std::process::Command::new(shell).args(args).exec().into())
    }

    /// Becomes the shell, as far as Windows allows.
    ///
    /// Windows has no `exec`, so the shell is spawned and waited on and this
    /// process stays as its parent — one idle process per pane, which is the
    /// cheaper of the two mistakes.
    #[cfg(windows)]
    fn exec(shell: &str, args: &[String]) -> anyhow::Result<()> {
        let status = std::process::Command::new(shell).args(args).status()?;

        std::process::exit(status.code().unwrap_or(0));
    }
}
