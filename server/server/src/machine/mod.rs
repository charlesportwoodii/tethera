mod address;
mod installed;
mod offer;

pub use address::MachineAddress;
pub use installed::Installed;
pub use offer::Offer;

use crate::config::ApplicationConfig;
use tethera_common::protocol::handshake::ServerInfo;
use tethera_common::structs::ids::{DeviceId, ServerId};

/// What this machine states about itself.
///
/// One place produces each value, because the id in a pairing QR and the id in
/// a `ServerHello` describe the same machine and a client that saw them differ
/// would have no way to tell which one it had paired to.
pub struct Machine;

impl Machine {
    /// How much of an endpoint id is enough to tell two machines apart on a
    /// screen, when nothing better than an endpoint id is available.
    pub const FALLBACK_LABEL_CHARS: usize = 12;

    /// Derived rather than generated. The endpoint id is already unique and
    /// already survives a restart, because `identity.key` is persisted, so a
    /// separately stored id could only ever drift away from it.
    pub fn server_id(endpoint_id: &str) -> ServerId {
        ServerId::mint(endpoint_id)
    }

    /// Derived from the endpoint id for the reason `endpoint_id_of` exists.
    pub fn device_id(endpoint_id: &str) -> DeviceId {
        DeviceId::mint(endpoint_id)
    }

    /// The endpoint id inside a device id, or `None` when it is not one.
    ///
    /// `Rpc::handle` serves `RevokeThisDevice` by passing `session.device.id` to
    /// `MachinePort::revoke`, which takes an endpoint id. Deriving one from the
    /// other is what reconciles them without editing a handler the protocol
    /// suite is written against.
    ///
    /// A value without the prefix is refused rather than used as an endpoint id.
    /// A second notion of device id already exists in this flow - `Device.id` is
    /// the database row id - and reading one as the other would look up a device
    /// whose endpoint id is "1".
    pub fn endpoint_id_of(device_id: &str) -> Option<&str> {
        device_id.strip_prefix(DeviceId::PREFIX)
    }

    pub fn info(config: &ApplicationConfig, endpoint_id: &str) -> ServerInfo {
        ServerInfo {
            id: Self::server_id(endpoint_id),
            label: Self::label(config, endpoint_id),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
        }
    }

    /// What a pairing screen calls this machine.
    ///
    /// A hostname is not a guess: it is what the machine is called, and it is
    /// what distinguishes two of them on a phone. The endpoint id prefix at the
    /// end of the chain is ugly and unambiguous, which is the property that
    /// matters once nothing better exists.
    pub fn label(config: &ApplicationConfig, endpoint_id: &str) -> String {
        config
            .label
            .as_deref()
            .and_then(Self::non_empty)
            .or_else(Self::hostname)
            .unwrap_or_else(|| endpoint_id.chars().take(Self::FALLBACK_LABEL_CHARS).collect())
    }

    fn hostname() -> Option<String> {
        for key in ["COMPUTERNAME", "HOSTNAME"] {
            if let Some(name) = std::env::var(key).ok().as_deref().and_then(Self::non_empty) {
                return Some(name);
            }
        }

        std::fs::read_to_string("/etc/hostname")
            .ok()
            .as_deref()
            .and_then(|raw| raw.lines().next())
            .and_then(Self::non_empty)
    }

    // A blank label renders as a name that happens to be invisible, which reads
    // as a broken client rather than as an unconfigured machine.
    fn non_empty(value: &str) -> Option<String> {
        let trimmed = value.trim();

        if trimmed.is_empty() {
            return None;
        }

        Some(trimmed.to_string())
    }
}
