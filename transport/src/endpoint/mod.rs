mod config;

pub use config::EndpointConfig;

use crate::alpn::Alpn;
use crate::error::TransportError;
use iroh::endpoint::{presets, Connection, RelayStatus};
use iroh::{Endpoint, EndpointAddr, EndpointId, RelayConfig, RelayMode, Watcher};
use std::net::{Ipv4Addr, SocketAddr};

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

        // IPv4 only. A router forward names an IPv4 port, and that is the whole
        // reason for pinning one; the IPv6 socket keeps its default ephemeral
        // bind, which iroh is allowed to fail on a host without IPv6.
        //
        // A port already in use fails the bind rather than falling back to an
        // ephemeral one. A silent fallback is the worse outcome: the forward
        // still points at the old port, direct connections quietly stop being
        // established, and everything keeps working over the relay with nothing
        // to say why it got slower.
        if let Some(port) = config.bind_port {
            builder = builder
                .bind_addr(SocketAddr::from((Ipv4Addr::UNSPECIFIED, port)))
                .map_err(|e| {
                    TransportError::Bind(format!("cannot bind udp port {port}: {e}"))
                })?;
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

    /// Tells iroh the network may have moved underneath it.
    ///
    /// iroh detects most changes itself, but not on every platform: `netwatch`
    /// widens its wall-clock poll to an hour on iOS and Android to spare the
    /// radio, and says in its own source that sleep detection does not work
    /// there as a result. A phone that was suspended comes back with expired NAT
    /// mappings and a dead relay socket, and iroh has nothing to tell it so.
    ///
    /// This asks it to look again. It is a hint and not a command: iroh compares
    /// the interfaces it finds against the ones it held, and a phone that
    /// resumed onto the same network with the same address is not a change it
    /// will act on.
    pub async fn network_change(&self) {
        self.endpoint.network_change().await;
    }

    /// Rebuilds the DNS resolver from the system configuration.
    ///
    /// The resolver reads the host's nameservers once, when it is built, and
    /// keeps them. A phone that moved network while this process was frozen
    /// comes back holding nameservers that are no longer reachable, and every
    /// lookup then fails with `no calls succeeded` — which takes the relay with
    /// it, because the relay is named by hostname. Nothing recovers on its own:
    /// iroh resets the resolver in exactly one place, behind the same major
    /// network change that a resumed phone does not report.
    ///
    /// Answers `false` when the endpoint has no resolver to reset, which is a
    /// closed endpoint.
    pub fn reset_dns(&self) -> bool {
        match self.endpoint.dns_resolver() {
            Ok(resolver) => {
                resolver.reset();

                true
            }
            Err(_) => false,
        }
    }

    /// The UDP sockets this endpoint is bound to right now.
    pub fn bound_sockets(&self) -> Vec<SocketAddr> {
        self.endpoint.bound_sockets()
    }

    /// Each home relay this endpoint knows, and whether it is connected.
    ///
    /// Empty until a relay has been selected, which is not the same as
    /// disconnected — see `Endpoint::home_relay_status`.
    pub fn home_relays(&self) -> Vec<RelayStatus> {
        let mut status = self.endpoint.home_relay_status();

        status.get()
    }

    /// Accepts one connection.
    ///
    /// The protocol is per-connection: the peer's endpoint id authenticates it,
    /// and the handshake and every later stream belong to that connection. A
    /// caller that only wanted a stream would have nothing to authenticate.
    pub async fn accept(&self) -> Result<Connection, TransportError> {
        let incoming = self
            .endpoint
            .accept()
            .await
            .ok_or(TransportError::EndpointClosed)?;

        incoming
            .await
            .map_err(|e| TransportError::Connection(e.to_string()))
    }

    pub async fn connect(
        &self,
        addr: impl Into<EndpointAddr>,
    ) -> Result<Connection, TransportError> {
        self.endpoint
            .connect(addr, Alpn::CURRENT)
            .await
            .map_err(|e| TransportError::Connection(e.to_string()))
    }

    pub async fn accept_bi(
        &self,
    ) -> Result<(iroh::endpoint::SendStream, iroh::endpoint::RecvStream), TransportError> {
        let connection = self.accept().await?;

        connection
            .accept_bi()
            .await
            .map_err(|e| TransportError::Connection(e.to_string()))
    }

    /// A dialable address for this endpoint on the loopback interface.
    ///
    /// Test-only, and deliberately not built from `addr()`: that reports what
    /// address discovery has found so far, which is empty for the first moments
    /// after binding and would make a loopback dial flaky. The bound socket is
    /// known immediately, and its wildcard host is rewritten to localhost
    /// because nothing can dial `0.0.0.0`.
    #[cfg(any(test, feature = "testing"))]
    pub fn loopback_addr(&self) -> Result<EndpointAddr, TransportError> {
        use std::net::{IpAddr, Ipv4Addr, SocketAddr};

        let bound = self
            .endpoint
            .bound_sockets()
            .into_iter()
            .find(|addr| addr.is_ipv4())
            .ok_or_else(|| TransportError::Bind("no bound IPv4 socket".to_string()))?;

        let dialable = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), bound.port());

        Ok(EndpointAddr::new(self.endpoint.id()).with_ip_addr(dialable))
    }

    /// Binds an endpoint with a fresh key and no relay.
    ///
    /// Test-only. Relays are disabled so a test never reaches the network: a
    /// suite that silently depends on n0's infrastructure fails for reasons that
    /// have nothing to do with the code under test.
    #[cfg(any(test, feature = "testing"))]
    pub async fn bind_local() -> Result<Self, TransportError> {
        let endpoint = Endpoint::builder(presets::N0)
            .secret_key(iroh::SecretKey::generate())
            .alpns(vec![Alpn::CURRENT.to_vec()])
            .relay_mode(RelayMode::Disabled)
            .bind()
            .await
            .map_err(|e| TransportError::Bind(e.to_string()))?;

        Ok(Self { endpoint })
    }
}
