use crate::config::ApplicationConfig;
use crate::services::PairingService;
use crate::storage::Storage;
use std::sync::Arc;

#[derive(clap::Args, Debug, Clone)]
pub struct Config {
    /// How long the issued code stays redeemable
    #[clap(long, default_value_t = PairingService::DEFAULT_TTL_SECONDS)]
    pub ttl_seconds: u64,
}

impl Config {
    pub async fn run(&self, config: Arc<ApplicationConfig>) -> anyhow::Result<()> {
        let connection = Storage::connect(&config).await?;
        let pairing = PairingService::new(Arc::new(connection));
        let (plaintext, superseded) = pairing
            .open_window(self.ttl_seconds, chrono::Utc::now().timestamp())
            .await?;

        // The only time this value is ever emitted. It is not logged, not
        // returned, and not recoverable from the row that was just written.
        println!("pairing code: {plaintext}");
        println!(
            "valid for {} seconds, and {} attempts",
            self.ttl_seconds,
            PairingService::DEFAULT_ATTEMPTS
        );

        // Opening a window closes any earlier one, so a code already on screen
        // has just stopped working.
        if superseded > 0 {
            println!("{superseded} earlier pairing window(s) were closed");
        }

        Ok(())
    }
}
