use crate::config::ApplicationConfig;
use crate::services::DeviceService;
use crate::storage::Storage;
use std::sync::Arc;
use tethera_common::structs::device::DeviceState;

/// Drops a device from the allow-list.
///
/// The operator's half of revocation. `RevokeThisDevice` lets a phone revoke
/// itself, which is no use at all when the phone is the thing that was lost.
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
                DeviceState::Revoked,
                chrono::Utc::now().timestamp(),
            )
            .await?;

        println!("revoked {} ({})", device.name, device.endpoint_id);

        // Revocation takes effect on the next connect. A live connection is not
        // closed, because closing one needs a registry the dispatcher does not
        // keep - said out loud so nobody assumes an in-flight session ended.
        println!("its next connection is refused; a session already open is not cut off");

        Ok(())
    }
}
