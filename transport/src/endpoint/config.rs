use iroh::{RelayUrl, SecretKey};

pub struct EndpointConfig {
    pub secret_key: SecretKey,
    pub relay: Option<(RelayUrl, String)>,
}

impl EndpointConfig {
    pub fn new(secret_key: SecretKey) -> Self {
        Self {
            secret_key,
            relay: None,
        }
    }

    pub fn with_relay(mut self, url: RelayUrl, auth_token: String) -> Self {
        self.relay = Some((url, auth_token));
        self
    }
}
