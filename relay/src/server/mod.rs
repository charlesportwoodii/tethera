use crate::access::SharedSecretAccess;
use crate::acme::AcmeService;
use crate::config::RelayConfig;
use iroh_relay::server::{QuicConfig, Server, ServerConfig, TlsConfig};
use std::sync::Arc;

pub mod tls;

pub use tls::TlsMaterial;

pub struct RelayServer {
    config: RelayConfig,
}

impl RelayServer {
    pub fn new(config: RelayConfig) -> Self {
        Self { config }
    }

    pub async fn run(self) -> anyhow::Result<()> {
        self.config.validate()?;

        let tls = TlsMaterial::from_config(&self.config)?;

        if tls.is_none() {
            if let Some(settings) = self.config.acme.clone() {
                AcmeService::new(settings).issue().await?;
            }
        }

        let mut server = Server::spawn(self.server_config(tls)?)
            .await
            .map_err(|error| anyhow::anyhow!("relay server did not start: {error}"))?;

        if let Some(addr) = server.http_addr() {
            tracing::info!(%addr, "relay listening on HTTP");
        }

        if let Some(addr) = server.https_addr() {
            tracing::info!(%addr, "relay listening on HTTPS");
        }

        if let Some(addr) = server.quic_addr() {
            tracing::info!(%addr, "relay listening on QUIC");
        }

        tokio::select! {
            biased;
            _ = tokio::signal::ctrl_c() => tracing::info!("shutdown requested"),
            _ = server.join() => tracing::warn!("relay stopped on its own"),
        }

        server
            .shutdown()
            .await
            .map_err(|error| anyhow::anyhow!("relay did not shut down cleanly: {error}"))
    }

    fn server_config(&self, tls: Option<TlsConfig>) -> anyhow::Result<ServerConfig> {
        let quic = self.quic_config(tls.is_some())?;

        let mut relay = iroh_relay::server::RelayConfig::new(self.config.http_bind);
        relay.access = Arc::new(SharedSecretAccess::new(self.config.secret.clone()));
        relay.tls = tls;

        let mut config = ServerConfig::default();
        config.relay = Some(relay);
        config.quic = quic;

        Ok(config)
    }

    // The QUIC listener has no certificate of its own; it borrows the relay's.
    // Without one it fails inside spawn, where the cause is no longer visible.
    fn quic_config(&self, has_tls: bool) -> anyhow::Result<Option<QuicConfig>> {
        let Some(bind) = self.config.quic_bind else {
            return Ok(None);
        };

        if !has_tls {
            anyhow::bail!(
                "quic_bind requires TLS; set tls_cert_path and tls_key_path or remove quic_bind"
            );
        }

        Ok(Some(QuicConfig::new(bind)))
    }
}
