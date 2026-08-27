use crate::config::ApplicationConfig;
use crate::services::DeviceService;
use crate::storage::Storage;
use std::sync::Arc;
use tethera_common::structs::device::Device;

/// Every device this machine knows, and what state it is in.
///
/// The only way an operator can confirm that a phone actually enrolled, which
/// makes it the first thing anybody reaches for when pairing looks wrong.
#[derive(clap::Args, Debug, Clone)]
pub struct Config {
    /// Print whole endpoint ids rather than the short form
    #[clap(long)]
    pub full: bool,
}

impl Config {
    /// Enough of an endpoint id to pick one machine's device out of a list, and
    /// enough to hand back to `device revoke`.
    pub const SHORT_ID_CHARS: usize = 12;

    pub async fn run(&self, config: Arc<ApplicationConfig>) -> anyhow::Result<()> {
        let connection = Arc::new(Storage::connect(&config).await?);
        let devices = DeviceService::new(connection.clone())
            .list(connection.as_ref())
            .await?;

        if devices.is_empty() {
            println!("no devices are enrolled");
            println!("run `tethera pair` on this machine, then scan the code with a phone");

            return Ok(());
        }

        println!(
            "{:<width$}  {:<10}  {:<20}  {}",
            "ENDPOINT",
            "STATE",
            "LAST SEEN",
            "NAME",
            width = self.id_width()
        );

        for device in &devices {
            println!(
                "{:<width$}  {:<10}  {:<20}  {}",
                self.identifier(device),
                device.state.as_str(),
                Self::when(device.last_seen_at),
                device.name,
                width = self.id_width()
            );
        }

        Ok(())
    }

    fn id_width(&self) -> usize {
        if self.full {
            64
        } else {
            Self::SHORT_ID_CHARS
        }
    }

    fn identifier(&self, device: &Device) -> String {
        if self.full {
            return device.endpoint_id.clone();
        }

        device
            .endpoint_id
            .chars()
            .take(Self::SHORT_ID_CHARS)
            .collect()
    }

    // Stored as epoch seconds. A device that has never connected says so rather
    // than showing the epoch.
    fn when(at: Option<i64>) -> String {
        let Some(at) = at else {
            return "never".to_string();
        };

        match chrono::DateTime::from_timestamp(at, 0) {
            Some(moment) => moment.format("%Y-%m-%d %H:%M:%S").to_string(),
            None => "unreadable".to_string(),
        }
    }
}
