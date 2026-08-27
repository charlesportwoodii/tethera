use crate::error::ClientError;
use iroh::endpoint::Connection;
use iroh::{EndpointAddr, EndpointId, RelayUrl};
use std::net::SocketAddr;
use std::str::FromStr;
use tethera_transport::endpoint::{EndpointConfig, TetheraEndpoint};

/// This device's Iroh endpoint.
///
/// One endpoint serves every paired machine. Iroh binds a UDP socket per
/// endpoint, so one per machine would mean a socket and a set of NAT keepalives
/// per machine on a phone.
pub struct ClientEndpoint {
    endpoint: TetheraEndpoint,
}

impl ClientEndpoint {
    pub async fn bind(config: EndpointConfig) -> Result<Self, ClientError> {
        let endpoint = TetheraEndpoint::bind(config)
            .await
            .map_err(|error| ClientError::Bind(error.to_string()))?;

        Ok(Self { endpoint })
    }

    pub fn id(&self) -> EndpointId {
        self.endpoint.id()
    }

    pub fn inner(&self) -> &TetheraEndpoint {
        &self.endpoint
    }

    /// Where to dial, from what a pairing offer or a remembered entry holds.
    ///
    /// An associated function taking plain values rather than a method, so every
    /// parsing decision below is testable without binding a socket.
    ///
    /// The two failure policies differ deliberately. A direct address is one of
    /// several and is only an optimisation, so a bad one is skipped and the rest
    /// are kept — otherwise a machine with one stale entry beside a good one
    /// becomes unreachable. A relay is the single value that makes a cold dial
    /// work from a mobile network, so a bad one is an error rather than a silent
    /// omission that would leave the machine looking dead with nothing to
    /// explain it.
    pub fn address(
        endpoint_id: &str,
        relay: Option<&str>,
        direct_addrs: &[String],
    ) -> Result<EndpointAddr, ClientError> {
        let id = EndpointId::from_str(endpoint_id).map_err(|error| ClientError::BadEndpointId {
            value: endpoint_id.to_string(),
            reason: error.to_string(),
        })?;

        let mut addr = EndpointAddr::new(id);

        if let Some(relay) = relay {
            let url = RelayUrl::from_str(relay).map_err(|error| ClientError::BadRelayUrl {
                value: relay.to_string(),
                reason: error.to_string(),
            })?;

            addr = addr.with_relay_url(url);
        }

        for candidate in direct_addrs {
            match SocketAddr::from_str(candidate) {
                Ok(socket) => addr = addr.with_ip_addr(socket),
                Err(error) => log::debug!("skipping direct address {candidate}: {error}"),
            }
        }

        Ok(addr)
    }

    pub async fn dial(
        &self,
        endpoint_id: &str,
        relay: Option<&str>,
        direct_addrs: &[String],
    ) -> Result<Connection, ClientError> {
        let addr = Self::address(endpoint_id, relay, direct_addrs)?;

        self.endpoint
            .connect(addr)
            .await
            .map_err(|error| ClientError::Dial(error.to_string()))
    }

    /// An endpoint with a fresh key and no relay.
    ///
    /// Test-only, and relays are disabled for the reason the transport crate
    /// gives: a suite that silently depends on n0's infrastructure fails for
    /// reasons that have nothing to do with the code under test.
    #[cfg(any(test, feature = "testing"))]
    pub async fn bind_local() -> Result<Self, ClientError> {
        let endpoint = TetheraEndpoint::bind_local()
            .await
            .map_err(|error| ClientError::Bind(error.to_string()))?;

        Ok(Self { endpoint })
    }
}
