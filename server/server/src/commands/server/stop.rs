use crate::commands::server::process::RunningProcess;
use crate::config::ApplicationConfig;
use crate::machine::MachineAddress;
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

        // A pidfile that outlived its process. Clearing it is the whole of what
        // stopping means here, and refusing to would leave the operator wedged:
        // `status` would keep reporting a dead server as running and no command
        // could ever say otherwise.
        if !RunningProcess::is_running(pid) {
            let _ = std::fs::remove_file(&pid_path);
            MachineAddress::clear(&config);

            println!("no server was running; cleared a stale pidfile for pid {pid}");

            return Ok(());
        }

        // Kept rather than raised. `taskkill /T` reports a failure when any
        // process in the tree refuses, and a server's children include ones it
        // spawned and abandoned - so it complains about a server it did kill.
        // The question this command answers is whether the server is gone, and
        // only the answer to that decides.
        let complaint = Self::signal(pid).err();

        // Signalling is not exiting. Unlinking the pidfile first would leave a
        // wedged server running with nothing left that can name it, and clearing
        // the address record would then make `tethera pair` report "no server is
        // running" about a server that is.
        if !RunningProcess::wait_until_gone(pid) {
            return Err(match complaint {
                Some(error) => error.context(format!(
                    "pid {pid} is still running; the pidfile is left in place so it can be \
                     stopped again"
                )),
                None => anyhow!(
                    "pid {pid} did not exit; the pidfile is left in place so it can be \
                     stopped again"
                ),
            });
        }

        let _ = std::fs::remove_file(&pid_path);

        // On Windows the kill is /F, so the server's own shutdown path never
        // runs and the addresses it published would outlive it until they went
        // stale. `tethera pair` reads that record to decide whether anything is
        // listening, and a stale answer there is a code typed into a screen
        // nothing is behind.
        MachineAddress::clear(&config);

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
        let mut command = std::process::Command::new("taskkill");
        command.args(["/PID", &pid.to_string(), "/T", "/F"]);

        let output = crate::process::Windowless::apply(&mut command).output()?;

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
