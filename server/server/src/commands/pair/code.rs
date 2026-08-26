use crate::config::ApplicationConfig;
use crate::storage::Storage;
use rand::Rng;
use sea_orm::{ActiveModelTrait, ActiveValue};
use std::sync::Arc;
use tethera_common::structs::pairing::PairingCode;

#[derive(clap::Args, Debug, Clone)]
pub struct Config {
    /// How long the issued code stays redeemable
    #[clap(long, default_value_t = 300)]
    pub ttl_seconds: u64,
}

impl Config {
    pub async fn run(&self, config: Arc<ApplicationConfig>) -> anyhow::Result<()> {
        let plaintext = Self::generate();
        let connection = Storage::connect(&config).await?;
        let expires_at = chrono::Utc::now().timestamp() + self.ttl_seconds as i64;

        tethera_entity::pairing_code::ActiveModel {
            code_hash: ActiveValue::Set(PairingCode::from_plaintext(&plaintext).to_hex()),
            expires_at: ActiveValue::Set(expires_at),
            consumed_at: ActiveValue::Set(None),
            ..Default::default()
        }
        .insert(&connection)
        .await?;

        // The only time this value is ever emitted. It is not logged, not
        // returned, and not recoverable from the row that was just written.
        println!("pairing code: {plaintext}");
        println!("valid for {} seconds", self.ttl_seconds);

        Ok(())
    }

    fn generate() -> String {
        format!("{:06}", rand::thread_rng().gen_range(0..1_000_000))
    }
}
