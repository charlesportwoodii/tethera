use crate::config::ApplicationConfig;
use crate::identity::Identity;
use iroh::{EndpointAddr, EndpointId, RelayUrl, SecretKey};
use tethera_transport::endpoint::{EndpointConfig, TetheraEndpoint};
use tethera_transport::error::TransportError;

pub struct TransportService {
    endpoint: TetheraEndpoint,
}

impl TransportService {
    pub async fn bind(config: &ApplicationConfig) -> anyhow::Result<Self> {
        let secret_key = Identity::load_or_create(&config.identity_path())?;
        let endpoint = TetheraEndpoint::bind(Self::endpoint_config(secret_key, config)?).await?;

        Ok(Self { endpoint })
    }

    pub fn id(&self) -> EndpointId {
        self.endpoint.id()
    }

    pub fn addr(&self) -> EndpointAddr {
        self.endpoint.addr()
    }

    pub async fn accept_bi(
        &self,
    ) -> Result<(iroh::endpoint::SendStream, iroh::endpoint::RecvStream), TransportError> {
        self.endpoint.accept_bi().await
    }

    // A relay is used only when both halves are configured. A URL without a
    // token cannot authenticate, and a token without a URL has no relay to
    // present itself to, so either alone is a misconfiguration rather than a
    // partial one.
    fn endpoint_config(
        secret_key: SecretKey,
        config: &ApplicationConfig,
    ) -> anyhow::Result<EndpointConfig> {
        let endpoint_config = EndpointConfig::new(secret_key);

        let (url, token) = match (&config.relay_url, &config.relay_token) {
            (Some(url), Some(token)) => (url, token),
            _ => return Ok(endpoint_config),
        };

        let url: RelayUrl = url
            .parse()
            .map_err(|error| anyhow::anyhow!("invalid relay url {url}: {error}"))?;

        Ok(endpoint_config.with_relay(url, token.clone()))
    }
}
