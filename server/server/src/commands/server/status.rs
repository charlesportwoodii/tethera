use crate::commands::server::process::RunningProcess;
use crate::config::ApplicationConfig;
use crate::identity::Identity;
use crate::machine::MachineAddress;
use std::sync::Arc;

#[derive(clap::Args, Debug, Clone)]
pub struct Config {}

impl Config {
    pub async fn run(&self, config: Arc<ApplicationConfig>) -> anyhow::Result<()> {
        match Identity::load_or_report_absent(&config.identity_path())? {
            Some(secret_key) => println!("endpoint id: {}", secret_key.public()),
            None => println!("endpoint id: none yet; the first server start creates it"),
        }

        println!("data dir:    {}", config.data_dir.display());

        println!("bind port:   {}", config.bind_port);

        // A pidfile survives a crash, so the file alone is not evidence. Saying
        // "yes" about a process that is gone sends an operator looking for a
        // server that is not there.
        match Self::read_pid(&config)? {
            Some(pid) if RunningProcess::is_running(pid) => {
                println!("running:     yes, pid {pid}")
            }
            Some(pid) => println!(
                "running:     no; a stale pidfile names pid {pid}. \
                 `tethera server stop` clears it"
            ),
            None => println!("running:     no"),
        }

        Self::report_reachability(&config);

        Ok(())
    }

    // Where the running server last said it could be reached. Absent or stale
    // means nothing is publishing it, which is the same signal `tethera pair`
    // uses to decide whether anything will answer a scanned code.
    fn report_reachability(config: &ApplicationConfig) {
        let now = chrono::Utc::now().timestamp();

        let Some(record) = MachineAddress::read(config).filter(|record| record.is_fresh(now))
        else {
            println!("reachable:   nothing published; no server is publishing addresses");

            return;
        };

        println!(
            "reachable:   {}",
            if record.direct_addrs.is_empty() {
                "no direct addresses yet".to_string()
            } else {
                record.direct_addrs.join(", ")
            }
        );

        if let Some(relay) = &record.relay {
            println!("relay:       {relay}");
        }
    }

    fn read_pid(config: &ApplicationConfig) -> anyhow::Result<Option<u32>> {
        let path = config.pid_path();

        if !path.exists() {
            return Ok(None);
        }

        Ok(std::fs::read_to_string(path)?.trim().parse::<u32>().ok())
    }
}
