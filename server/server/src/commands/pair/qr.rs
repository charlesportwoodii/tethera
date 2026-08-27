use crate::config::ApplicationConfig;
use crate::identity::Identity;
use crate::machine::Offer;
use std::sync::Arc;

#[derive(clap::Args, Debug, Clone)]
pub struct Config {}

impl Config {
    pub async fn run(&self, config: Arc<ApplicationConfig>) -> anyhow::Result<()> {
        let endpoint_id = Identity::load_or_create(&config.identity_path())?
            .public()
            .to_string();

        // No window is opened here. The offer is an address, and an address
        // that admitted a stranger would make the window pointless.
        let uri = Offer::build(&config, &endpoint_id, chrono::Utc::now().timestamp()).to_uri();

        println!("{}", Offer::qr(&uri)?);
        println!("{uri}");

        Ok(())
    }
}
