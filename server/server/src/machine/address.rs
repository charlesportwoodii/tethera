use crate::config::ApplicationConfig;
use iroh::{EndpointAddr, TransportAddr};
use serde::{Deserialize, Serialize};

/// Where this machine can be reached, as the running server last saw it.
///
/// `tethera pair` has no channel to the server and must not bind an endpoint of
/// its own: a second endpoint holding the same secret key is two endpoints with
/// one identity. So the server writes this record while it runs and `pair`
/// reads it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachineAddress {
    pub endpoint_id: String,
    pub direct_addrs: Vec<String>,
    pub relay: Option<String>,
    /// Seconds since the Unix epoch, matching every stored timestamp.
    pub updated_at: i64,
}

impl MachineAddress {
    /// Three heartbeats. The record is rewritten on a timer rather than only on
    /// change, which is what lets a reader tell a running server from a dead
    /// one without a second liveness channel.
    pub const STALE_AFTER_SECONDS: i64 = 90;

    pub fn new(
        endpoint_id: String,
        direct_addrs: Vec<String>,
        relay: Option<String>,
        updated_at: i64,
    ) -> Self {
        Self {
            endpoint_id,
            direct_addrs,
            relay,
            updated_at,
        }
    }

    pub fn from_endpoint_addr(addr: &EndpointAddr, updated_at: i64) -> Self {
        let mut direct_addrs = Vec::new();
        let mut relay = None;

        for transport in &addr.addrs {
            match transport {
                TransportAddr::Ip(socket) => direct_addrs.push(socket.to_string()),
                TransportAddr::Relay(url) => {
                    relay.get_or_insert_with(|| url.to_string());
                }
                _ => {}
            }
        }

        Self::new(addr.id.to_string(), direct_addrs, relay, updated_at)
    }

    pub fn publish(&self, config: &ApplicationConfig) -> anyhow::Result<()> {
        config.ensure_data_dir()?;
        std::fs::write(config.endpoint_path(), serde_json::to_vec_pretty(self)?)?;

        Ok(())
    }

    /// An unreadable record is an absent one. A half-written file means the
    /// server was interrupted, and guessing at its contents would put addresses
    /// in a QR that nothing answers on.
    pub fn read(config: &ApplicationConfig) -> Option<Self> {
        let raw = std::fs::read(config.endpoint_path()).ok()?;

        serde_json::from_slice(&raw).ok()
    }

    pub fn is_fresh(&self, now: i64) -> bool {
        now - self.updated_at <= Self::STALE_AFTER_SECONDS
    }

    pub fn clear(config: &ApplicationConfig) {
        let _ = std::fs::remove_file(config.endpoint_path());
    }
}
