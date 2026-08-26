mod config;

pub use config::EndpointConfig;

use crate::alpn::Alpn;
use crate::error::TransportError;
use iroh::endpoint::presets;
use iroh::{Endpoint, EndpointAddr, EndpointId, RelayConfig, RelayMode};

pub struct TetheraEndpoint {
    endpoint: Endpoint,
}

impl TetheraEndpoint {
    pub async fn bind(config: EndpointConfig) -> Result<Self, TransportError> {
        let mut builder = Endpoint::builder(presets::N0)
            .secret_key(config.secret_key)
            .alpns(vec![Alpn::CURRENT.to_vec()]);

        if let Some((url, token)) = config.relay {
            let relay = RelayConfig::new(url, None).with_auth_token(token);
            builder = builder.relay_mode(RelayMode::Custom(relay.into()));
        }

        let endpoint = builder
            .bind()
            .await
            .map_err(|e| TransportError::Bind(e.to_string()))?;

        Ok(Self { endpoint })
    }

    pub fn id(&self) -> EndpointId {
        self.endpoint.id()
    }

    pub fn addr(&self) -> EndpointAddr {
        self.endpoint.addr()
    }

    pub fn inner(&self) -> &Endpoint {
        &self.endpoint
    }

    pub async fn accept_bi(
        &self,
    ) -> Result<(iroh::endpoint::SendStream, iroh::endpoint::RecvStream), TransportError> {
        let incoming = self
            .endpoint
            .accept()
            .await
            .ok_or(TransportError::EndpointClosed)?;

        let connection = incoming
            .await
            .map_err(|e| TransportError::Connection(e.to_string()))?;

        connection
            .accept_bi()
            .await
            .map_err(|e| TransportError::Connection(e.to_string()))
    }
}
