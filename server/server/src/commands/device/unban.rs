use crate::config::ApplicationConfig;
use crate::services::DeviceService;
use crate::storage::Storage;
use std::sync::Arc;
use tethera_common::structs::device::DeviceState;

/// Lifts a ban, returning the device to `Pending`.
///
/// Never straight back to `Active`: a lifted ban puts the device at the start of
/// pairing, where it needs a fresh code like any other.
#[derive(clap::Args, Debug, Clone)]
pub struct Config {
    /// The device's endpoint id, or enough of the front of it to be unambiguous
    pub id: String,
}

impl Config {
    pub async fn run(&self, config: Arc<ApplicationConfig>) -> anyhow::Result<()> {
        let connection = Arc::new(Storage::connect(&config).await?);
        let devices = DeviceService::new(connection.clone());
        let device = devices.resolve(connection.as_ref(), &self.id).await?;

        devices
            .set_state(
                connection.as_ref(),
                &device.endpoint_id,
                DeviceState::Pending,
                chrono::Utc::now().timestamp(),
            )
            .await?;

        println!("lifted the ban on {} ({})", device.name, device.endpoint_id);
        println!("it is pending, and must pair again: run `tethera pair`");

        Ok(())
    }
}
