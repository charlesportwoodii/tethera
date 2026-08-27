mod code;

pub use code::PairingCode;

use crate::errors::TetheraError;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct PairingOffer {
    pub server_id: String,
    pub endpoint_id: Option<String>,
    /// Where a client that has never met this machine reaches it. Absent when
    /// the machine is configured without one.
    pub relay: Option<String>,
    pub direct_addrs: Vec<String>,
    /// A human name for the machine, so a pairing screen shows more than 64 hex
    /// characters. Absent rather than empty when nothing is configured: a blank
    /// string would render as a name that happens to be invisible.
    pub label: Option<String>,
}

impl PairingOffer {
    pub const SCHEME: &'static str = "tethera";
    pub const HOST: &'static str = "pair";

    pub fn new(
        server_id: String,
        endpoint_id: Option<String>,
        relay: Option<String>,
        direct_addrs: Vec<String>,
        label: Option<String>,
    ) -> Self {
        Self {
            server_id,
            endpoint_id,
            relay,
            direct_addrs,
            label,
        }
    }

    pub fn to_uri(&self) -> String {
        let mut uri = format!(
            "{}://{}?s={}",
            Self::SCHEME,
            Self::HOST,
            Self::encode(&self.server_id)
        );

        if let Some(endpoint_id) = &self.endpoint_id {
            uri.push_str(&format!("&n={}", Self::encode(endpoint_id)));
        }

        if let Some(relay) = &self.relay {
            uri.push_str(&format!("&r={}", Self::encode(relay)));
        }

        if !self.direct_addrs.is_empty() {
            uri.push_str(&format!(
                "&a={}",
                Self::encode(&self.direct_addrs.join(","))
            ));
        }

        if let Some(label) = &self.label {
            uri.push_str(&format!("&l={}", Self::encode(label)));
        }

        uri
    }

    pub fn from_uri(uri: &str) -> Result<Self, TetheraError> {
        let parsed =
            Url::parse(uri).map_err(|e| TetheraError::InvalidPairingUri(e.to_string()))?;

        if parsed.scheme() != Self::SCHEME {
            return Err(TetheraError::InvalidPairingUri(format!(
                "expected scheme {}, got {}",
                Self::SCHEME,
                parsed.scheme()
            )));
        }

        // The host is the operation. Accepting any host here would let a
        // future tethera://revoke deeplink be parsed as a pairing offer.
        if parsed.host_str() != Some(Self::HOST) {
            return Err(TetheraError::InvalidPairingUri(format!(
                "expected host {}, got {}",
                Self::HOST,
                parsed.host_str().unwrap_or("none")
            )));
        }

        let mut server_id = None;
        let mut endpoint_id = None;
        let mut relay = None;
        let mut direct_addrs = Vec::new();
        let mut label = None;

        for (key, value) in parsed.query_pairs() {
            match key.as_ref() {
                "s" => server_id = Some(value.into_owned()),
                "n" => endpoint_id = Some(value.into_owned()),
                "r" => relay = Some(value.into_owned()),
                "l" => label = Some(value.into_owned()),
                "a" => {
                    direct_addrs = value
                        .split(',')
                        .filter(|s| !s.is_empty())
                        .map(|s| s.to_string())
                        .collect()
                }
                _ => {}
            }
        }

        let server_id = server_id.ok_or_else(|| {
            TetheraError::InvalidPairingUri("missing required field s".to_string())
        })?;

        Ok(Self {
            server_id,
            endpoint_id,
            relay,
            direct_addrs,
            label,
        })
    }

    fn encode(value: &str) -> String {
        url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
    }
}
