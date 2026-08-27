/// Whether a pid is still a live process.
///
/// A pidfile outlives a crash, so every command that reads one has to ask this
/// before believing it. `status` reporting a dead server as running and `stop`
/// refusing to clean up after one are the same defect from two directions.
pub struct RunningProcess;

impl RunningProcess {
    /// How long to wait for a signalled process to actually go.
    pub const EXIT_POLLS: u32 = 50;
    pub const EXIT_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(100);

    pub fn wait_until_gone(pid: u32) -> bool {
        for _ in 0..Self::EXIT_POLLS {
            if !Self::is_running(pid) {
                return true;
            }

            std::thread::sleep(Self::EXIT_POLL_INTERVAL);
        }

        !Self::is_running(pid)
    }

    // taskkill /F has already terminated the process by the time it reports
    // success, so this is a confirmation rather than a wait.
    #[cfg(windows)]
    pub fn is_running(pid: u32) -> bool {
        let mut command = std::process::Command::new("tasklist");
        command.args(["/FI", &format!("PID eq {pid}"), "/NH"]);

        crate::process::Windowless::apply(&mut command)
            .output()
            .map(|output| String::from_utf8_lossy(&output.stdout).contains(&pid.to_string()))
            .unwrap_or(false)
    }

    // Signal 0 delivers nothing and reports whether the process is still there.
    #[cfg(not(windows))]
    pub fn is_running(pid: u32) -> bool {
        unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
    }
}
