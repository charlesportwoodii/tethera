use std::path::PathBuf;
use std::sync::Arc;

use crate::config::{ApplicationConfig, HerdrConfig};

/// `tethera herdr` — the one place tethera writes herdr's own configuration.
///
/// A shim reaches a pane only by being that pane's shell, and herdr decides that
/// from `[terminal] default_shell`. There is no `herdr config set`, so this is a
/// read-modify-write on a file tethera does not own — which is why it backs up
/// first, validates through herdr itself afterwards, and can be reverted.
#[derive(clap::Args, Debug, Clone)]
pub struct Config {
    #[clap(subcommand)]
    pub cmd: SubCommand,
}

#[derive(clap::Subcommand, Debug, Clone)]
pub enum SubCommand {
    /// Point herdr's `default_shell` at the tethera shim, so every new pane
    /// streams
    Hook(HookArgs),
}

#[derive(clap::Args, Debug, Clone)]
pub struct HookArgs {
    /// The shim to install. Defaults to this binary
    #[clap(long)]
    pub shim: Option<PathBuf>,

    /// Remove the hook instead, returning `default_shell` to unset
    #[clap(long)]
    pub remove: bool,

    /// Print what would change without writing anything
    #[clap(long)]
    pub dry_run: bool,
}

impl Config {
    pub async fn run(&self, _config: Arc<ApplicationConfig>) -> anyhow::Result<()> {
        match &self.cmd {
            SubCommand::Hook(args) => Self::hook(args),
        }
    }

    fn hook(args: &HookArgs) -> anyhow::Result<()> {
        let path = HerdrConfig::path()
            .ok_or_else(|| anyhow::anyhow!("cannot work out where herdr keeps its configuration"))?;

        // Never created. A missing config means herdr has not run here, and
        // writing one would be tethera inventing another tool's settings.
        if !path.exists() {
            anyhow::bail!(
                "no herdr configuration at {}. run herdr once so it writes its own, then try again",
                path.display()
            );
        }

        let before = std::fs::read_to_string(&path)?;

        let after = if args.remove {
            HerdrConfig::unhook(&before)
        } else {
            let shim = match &args.shim {
                Some(named) => named.clone(),
                None => std::env::current_exe()?,
            };

            if let Err(refusal) = HerdrConfig::installable(&shim) {
                anyhow::bail!(refusal);
            }

            HerdrConfig::hook(&before, &shim)
        };

        if after == before {
            println!("herdr's configuration already says what you asked for; nothing written");

            return Ok(());
        }

        if args.dry_run {
            match HerdrConfig::hooked(&after) {
                Some(shell) => println!("would start new panes with {shell}"),
                None => println!("would return new panes to herdr's own default shell"),
            }

            println!("nothing was written");

            return Ok(());
        }

        // The configuration as tethera first found it, written once and never
        // overwritten. A second hook would otherwise back up a document that
        // already names the shim, and an operator restoring that file to undo
        // the hook reinstalls it instead — which is exactly how this machine
        // was broken twice: the backup was restored, and the hook came back
        // with it.
        //
        // Rollback below does not read this file. It restores from `before`,
        // held in memory, so keeping the older backup costs nothing.
        let backup = path.with_extension(format!("toml{}", HerdrConfig::BACKUP_SUFFIX));
        let kept = backup.exists();

        if !kept {
            std::fs::write(&backup, &before)?;
        }

        std::fs::write(&path, &after)?;

        // Asked of herdr rather than assumed. tethera's idea of a valid herdr
        // config is not authoritative, and the operator's terminal is what pays
        // for the difference.
        if let Err(error) = Self::verify() {
            std::fs::write(&path, &before)?;

            anyhow::bail!(
                "herdr refused the edited configuration, so it was put back. {error:#}"
            );
        }

        Self::reload();

        match HerdrConfig::hooked(&after) {
            Some(shell) => println!("herdr will start new panes with {shell}"),
            None => println!("herdr will start new panes with its own default shell again"),
        }

        match kept {
            true => println!(
                "the configuration from before tethera first touched it is still at {}",
                backup.display()
            ),
            false => println!("the previous configuration is at {}", backup.display()),
        }

        println!("panes already open keep the shell they started with; reopen one to stream it");

        Ok(())
    }

    /// `herdr config check`, which is herdr's own opinion of the file.
    fn verify() -> anyhow::Result<()> {
        let output = std::process::Command::new("herdr")
            .args(["config", "check"])
            .output()?;

        if output.status.success() {
            return Ok(());
        }

        anyhow::bail!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout).trim(),
            String::from_utf8_lossy(&output.stderr).trim()
        )
    }

    /// Asks a running herdr to re-read what was just written.
    ///
    /// Best effort and reported quietly: herdr may not be running, and the edit
    /// is on disk either way. A failure here costs a restart, not the change.
    fn reload() {
        let reloaded = std::process::Command::new("herdr")
            .args(["server", "reload-config"])
            .output();

        match reloaded {
            Ok(output) if output.status.success() => {}
            _ => println!("herdr did not reload; restart it to pick this up"),
        }
    }
}
