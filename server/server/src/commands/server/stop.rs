use crate::config::ApplicationConfig;
use anyhow::anyhow;
use std::path::Path;
use std::sync::Arc;

#[derive(clap::Args, Debug, Clone)]
pub struct Config {}

impl Config {
    pub async fn run(&self, config: Arc<ApplicationConfig>) -> anyhow::Result<()> {
        let pid_path = config.pid_path();

        // Stopping a server that is already stopped is the state the caller
        // asked for, not a failure.
        let Some(pid) = Self::read_pid(&pid_path)? else {
            println!("no running server");
            return Ok(());
        };

        Self::signal(pid)?;
        let _ = std::fs::remove_file(&pid_path);

        println!("stopped tethera server, pid {pid}");

        Ok(())
    }

    fn read_pid(path: &Path) -> anyhow::Result<Option<u32>> {
        if !path.exists() {
            return Ok(None);
        }

        let raw = std::fs::read_to_string(path)?;

        raw.trim()
            .parse::<u32>()
            .map(Some)
            .map_err(|error| anyhow!("{} does not hold a pid: {error}", path.display()))
    }

    // taskkill without /F asks a process to close through its window, and a
    // detached server has no window to ask, so the request is refused rather
    // than delivered.
    #[cfg(windows)]
    fn signal(pid: u32) -> anyhow::Result<()> {
        let output = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .output()?;

        if !output.status.success() {
            return Err(anyhow!(
                "taskkill failed for pid {pid}: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }

        Ok(())
    }

    #[cfg(not(windows))]
    fn signal(pid: u32) -> anyhow::Result<()> {
        let result = unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) };

        if result != 0 {
            return Err(anyhow!(
                "cannot signal pid {pid}: {}",
                std::io::Error::last_os_error()
            ));
        }

        Ok(())
    }
}
