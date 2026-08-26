use serde::Deserialize;
use std::fmt;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

pub mod acme_settings;

pub use acme_settings::AcmeSettings;

#[derive(Clone, Deserialize)]
pub struct RelayConfig {
    #[serde(default = "RelayConfig::default_http_bind")]
    pub http_bind: SocketAddr,
    #[serde(default)]
    pub https_bind: Option<SocketAddr>,
    #[serde(default)]
    pub quic_bind: Option<SocketAddr>,
    pub secret: String,
    #[serde(default)]
    pub tls_cert_path: Option<PathBuf>,
    #[serde(default)]
    pub tls_key_path: Option<PathBuf>,
    #[serde(default)]
    pub acme: Option<AcmeSettings>,
}

impl RelayConfig {
    pub const DEFAULT_HTTP_BIND: &'static str = "0.0.0.0:8080";
    pub const DEFAULT_HTTPS_BIND: &'static str = "0.0.0.0:443";

    fn default_http_bind() -> SocketAddr {
        Self::DEFAULT_HTTP_BIND.parse().expect("valid default bind")
    }

    pub fn default_https_bind() -> SocketAddr {
        Self::DEFAULT_HTTPS_BIND
            .parse()
            .expect("valid default bind")
    }

    // The TOML error is not propagated. Its Display carries a snippet of the
    // offending source line, so a malformed `secret = ...` would print the
    // secret to stderr on the way out.
    pub fn from_file(path: &Path) -> anyhow::Result<Self> {
        let raw = std::fs::read_to_string(path)?;

        let mut config: Self = toml::from_str(&raw).map_err(|error| {
            anyhow::anyhow!(
                "{} is not valid TOML at line {}",
                path.display(),
                Self::line_of(&raw, error.span().map(|span| span.start))
            )
        })?;

        // Trimmed once, here, so validate and SharedSecretAccess agree about
        // the value. A secret that validates untrimmed and is enforced with
        // its spaces presents as a relay that never works.
        config.secret = config.secret.trim().to_string();

        Ok(config)
    }

    fn line_of(raw: &str, offset: Option<usize>) -> usize {
        match offset {
            Some(offset) => raw[..offset.min(raw.len())].lines().count().max(1),
            None => 1,
        }
    }

    // A relay whose secret is empty admits any caller that sends an empty
    // token, which is every caller that sends none at all under a different
    // name. Refusing at startup is the only place this is visible.
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.secret.trim().is_empty() {
            anyhow::bail!("secret must not be empty; every server and client must send the same value");
        }

        Ok(())
    }
}

// Hand written so a `tracing::debug!(?config)` cannot dump a live credential
// into the log.
impl fmt::Debug for RelayConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RelayConfig")
            .field("http_bind", &self.http_bind)
            .field("https_bind", &self.https_bind)
            .field("quic_bind", &self.quic_bind)
            .field("secret", &"<redacted>")
            .field("tls_cert_path", &self.tls_cert_path)
            .field("tls_key_path", &self.tls_key_path)
            .field("acme", &self.acme)
            .finish()
    }
}

impl Default for RelayConfig {
    fn default() -> Self {
        Self {
            http_bind: Self::default_http_bind(),
            https_bind: None,
            quic_bind: None,
            secret: String::new(),
            tls_cert_path: None,
            tls_key_path: None,
            acme: None,
        }
    }
}
