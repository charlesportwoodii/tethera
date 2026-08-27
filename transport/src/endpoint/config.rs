use iroh::{RelayUrl, SecretKey};

pub struct EndpointConfig {
    pub secret_key: SecretKey,
    pub relay: Option<(RelayUrl, String)>,
    /// The UDP port to bind IPv4 on.
    ///
    /// `None` lets the operating system choose, which is right for a client: a
    /// phone forwards nothing, and two clients on one machine would collide.
    ///
    /// A machine that wants direct connections from outside its network sets
    /// one, because a router forward names a fixed port and an endpoint that
    /// moved to a new port on every restart could never be forwarded to.
    pub bind_port: Option<u16>,
}

impl EndpointConfig {
    pub fn new(secret_key: SecretKey) -> Self {
        Self {
            secret_key,
            relay: None,
            bind_port: None,
        }
    }

    pub fn with_relay(mut self, url: RelayUrl, auth_token: String) -> Self {
        self.relay = Some((url, auth_token));
        self
    }

    pub fn with_bind_port(mut self, port: u16) -> Self {
        self.bind_port = Some(port);
        self
    }
}
