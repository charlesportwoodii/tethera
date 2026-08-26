use crate::config::ApplicationConfig;
use crate::runtime::ServerRuntime;
use std::sync::Arc;

#[derive(clap::Args, Debug, Clone)]
pub struct Config {
    /// Run in the background instead of holding the terminal
    #[clap(long)]
    pub detach: bool,
}

impl Config {
    pub async fn run(&self, config: Arc<ApplicationConfig>) -> anyhow::Result<()> {
        if self.detach {
            return Self::respawn_detached(&config);
        }

        std::fs::write(config.pid_path(), std::process::id().to_string())?;

        let result = ServerRuntime::new(config.clone()).start().await;

        let _ = std::fs::remove_file(config.pid_path());

        result
    }

    // The child is given the resolved data dir rather than inheriting the
    // caller's environment, so a detached server keeps the state the operator
    // asked for instead of falling back to the platform default.
    fn detached_arguments(config: &ApplicationConfig) -> [std::ffi::OsString; 4] {
        [
            std::ffi::OsString::from("--data-dir"),
            config.data_dir.clone().into_os_string(),
            std::ffi::OsString::from("server"),
            std::ffi::OsString::from("start"),
        ]
    }

    // Windows has no fork, so the double-fork daemon idiom is unavailable and
    // detaching is expressed as creation flags on a fresh process instead.
    #[cfg(windows)]
    fn respawn_detached(config: &ApplicationConfig) -> anyhow::Result<()> {
        use std::os::windows::process::CommandExt;

        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;

        let child = std::process::Command::new(std::env::current_exe()?)
            .args(Self::detached_arguments(config))
            .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)
            .spawn()?;

        std::fs::write(config.pid_path(), child.id().to_string())?;
        println!("tethera server started, pid {}", child.id());

        Ok(())
    }

    #[cfg(not(windows))]
    fn respawn_detached(config: &ApplicationConfig) -> anyhow::Result<()> {
        use std::os::unix::process::CommandExt;

        let mut command = std::process::Command::new(std::env::current_exe()?);
        command.args(Self::detached_arguments(config));

        unsafe {
            command.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }

        let child = command.spawn()?;

        std::fs::write(config.pid_path(), child.id().to_string())?;
        println!("tethera server started, pid {}", child.id());

        Ok(())
    }
}
