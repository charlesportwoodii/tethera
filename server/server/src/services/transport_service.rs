use crate::config::ApplicationConfig;
use crate::identity::Identity;
use iroh::{EndpointAddr, EndpointId, RelayUrl, SecretKey};
use tethera_transport::endpoint::{EndpointConfig, TetheraEndpoint};
use tethera_transport::error::TransportError;

pub struct TransportService {
    endpoint: TetheraEndpoint,
}

impl TransportService {
    pub fn new(endpoint: TetheraEndpoint) -> Self {
        Self { endpoint }
    }

    pub async fn bind(config: &ApplicationConfig) -> anyhow::Result<Self> {
        let secret_key = Identity::load_or_create(&config.identity_path())?;
        let endpoint = TetheraEndpoint::bind(Self::endpoint_config(secret_key, config)?).await;

        // The port is fixed so a router forward can name it, which means the
        // common failure is a second server already holding it. iroh reports
        // "Failed to bind sockets", which names neither the port nor the likely
        // cause, and this is the error an operator meets most often.
        let endpoint = endpoint.map_err(|error| {
            anyhow::anyhow!(
                "cannot bind udp port {}: {error}. another tethera server is probably \
                 already running - stop it with `tethera server stop`, or give this one \
                 a different port with --bind-port",
                config.bind_port
            )
        })?;

        Ok(Self::new(endpoint))
    }

    pub fn endpoint(&self) -> &TetheraEndpoint {
        &self.endpoint
    }

    /// Accepts one connection.
    ///
    /// The protocol is per-connection: the peer's endpoint id is proved by TLS
    /// on the connection, the handshake belongs to it, and every later stream
    /// is served under what that handshake decided.
    pub async fn accept(&self) -> Result<iroh::endpoint::Connection, TransportError> {
        self.endpoint.accept().await
    }

    pub fn id(&self) -> EndpointId {
        self.endpoint.id()
    }

    pub fn addr(&self) -> EndpointAddr {
        self.endpoint.addr()
    }

    // A relay is used only when both halves are configured. A URL without a
    // token cannot authenticate, and a token without a URL has no relay to
    // present itself to, so either alone is a misconfiguration rather than a
    // partial one.
    fn endpoint_config(
        secret_key: SecretKey,
        config: &ApplicationConfig,
    ) -> anyhow::Result<EndpointConfig> {
        let endpoint_config = EndpointConfig::new(secret_key).with_bind_port(config.bind_port);

        let (url, token) = match (&config.relay_url, &config.relay_token) {
            (Some(url), Some(token)) => (url, token),
            // Said out loud rather than ignored. Half a relay configuration is
            // a machine that silently falls back to the default relays, and the
            // operator who set the flag has no way to tell.
            (Some(url), None) => {
                tracing::warn!(
                    %url,
                    "a relay url is set with no shared secret; set TETHERA_RELAY_TOKEN to the same value the relay was started with, or this machine uses the default relays"
                );

                return Ok(endpoint_config);
            }
            (None, Some(_)) => {
                tracing::warn!(
                    "a relay secret is set with no relay url; set --relay-url or TETHERA_RELAY_URL, or this machine uses the default relays"
                );

                return Ok(endpoint_config);
            }
            (None, None) => return Ok(endpoint_config),
        };

        let url: RelayUrl = url
            .parse()
            .map_err(|error| anyhow::anyhow!("invalid relay url {url}: {error}"))?;

        Ok(endpoint_config.with_relay(url, token.clone()))
    }
}
