use crate::config::AcmeSettings;

pub struct AcmeService {
    settings: AcmeSettings,
}

impl AcmeService {
    pub fn new(settings: AcmeSettings) -> Self {
        Self { settings }
    }

    pub fn settings(&self) -> &AcmeSettings {
        &self.settings
    }

    // iroh-relay ships ACME through tokio-rustls-acme, which is TLS-ALPN-01.
    // DNS-01 against Cloudflare therefore cannot use CertConfig::LetsEncrypt
    // and has to issue here, feeding CertConfig::Manual.
    pub async fn issue(&self) -> anyhow::Result<()> {
        anyhow::bail!(
            "DNS-01 issuance for {:?} is not implemented; supply tls_cert_path and tls_key_path",
            self.settings.domains
        )
    }
}
