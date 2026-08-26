use crate::config::RelayConfig;
use anyhow::Context;
use iroh_relay::server::{CertConfig, TlsConfig};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

pub struct TlsMaterial;

impl TlsMaterial {
    // Both paths or neither. A half-configured pair that quietly fell back to
    // plain HTTP would look healthy everywhere except on the wire.
    pub fn from_config(config: &RelayConfig) -> anyhow::Result<Option<TlsConfig>> {
        let (cert_path, key_path) = match (&config.tls_cert_path, &config.tls_key_path) {
            (Some(cert), Some(key)) => (cert, key),
            (None, None) => return Ok(None),
            (Some(_), None) => anyhow::bail!(
                "tls_cert_path is set but tls_key_path is not; set both or neither"
            ),
            (None, Some(_)) => anyhow::bail!(
                "tls_key_path is set but tls_cert_path is not; set both or neither"
            ),
        };

        let server_config = Self::server_config(cert_path, key_path)?;
        let https_bind = config
            .https_bind
            .unwrap_or_else(RelayConfig::default_https_bind);

        Ok(Some(TlsConfig::new(
            https_bind,
            CertConfig::Manual { server_config },
        )))
    }

    fn server_config(
        cert_path: &Path,
        key_path: &Path,
    ) -> anyhow::Result<rustls::ServerConfig> {
        let certs = Self::certificates(cert_path)?;
        let key = Self::private_key(key_path)?;

        rustls::ServerConfig::builder_with_provider(std::sync::Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .context("protocol versions supported by ring")?
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .context("certificate and key do not form a usable TLS configuration")
    }

    fn certificates(path: &Path) -> anyhow::Result<Vec<CertificateDer<'static>>> {
        let file = File::open(path)
            .with_context(|| format!("cannot read tls_cert_path {}", path.display()))?;
        let certs = rustls_pemfile::certs(&mut BufReader::new(file))
            .collect::<Result<Vec<_>, _>>()
            .with_context(|| format!("cannot parse certificates in {}", path.display()))?;

        if certs.is_empty() {
            anyhow::bail!("no certificates found in {}", path.display());
        }

        Ok(certs)
    }

    fn private_key(path: &Path) -> anyhow::Result<PrivateKeyDer<'static>> {
        let file = File::open(path)
            .with_context(|| format!("cannot read tls_key_path {}", path.display()))?;

        rustls_pemfile::private_key(&mut BufReader::new(file))
            .with_context(|| format!("cannot parse a private key in {}", path.display()))?
            .with_context(|| format!("no private key found in {}", path.display()))
    }
}
