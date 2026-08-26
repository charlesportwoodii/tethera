use serde::Deserialize;
use std::fmt;
use std::path::PathBuf;

#[derive(Clone, Deserialize)]
pub struct AcmeSettings {
    pub domains: Vec<String>,
    pub contact: Vec<String>,
    pub cloudflare_zone_id: String,
    pub cloudflare_api_token: String,
    pub cache_dir: PathBuf,
}

// Hand written so a `tracing::debug!(?config)` cannot dump a live credential
// into the log.
impl fmt::Debug for AcmeSettings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AcmeSettings")
            .field("domains", &self.domains)
            .field("contact", &self.contact)
            .field("cloudflare_zone_id", &self.cloudflare_zone_id)
            .field("cloudflare_api_token", &"<redacted>")
            .field("cache_dir", &self.cache_dir)
            .finish()
    }
}
