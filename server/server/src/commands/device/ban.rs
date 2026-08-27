use crate::config::ApplicationConfig;
use crate::services::DeviceService;
use crate::storage::Storage;
use std::sync::Arc;
use tethera_common::structs::device::DeviceState;

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
                DeviceState::Banned,
                chrono::Utc::now().timestamp(),
            )
            .await?;

        println!("banned {} ({})", device.name, device.endpoint_id);
        println!("lift it with `tethera device unban`, which returns it to pending");

        Ok(())
    }
}
