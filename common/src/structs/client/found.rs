use crate::protocol::handshake::{CodeFormat, ServerInfo};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The machine that answered a pairing dial, before any code is typed.
///
/// `server` comes from `ServerHello::EnrollPending`, so QUIC TLS has already
/// proved the endpoint id behind it. That is what lets a pairing screen name the
/// machine before a person commits to anything.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct FoundServer {
    pub server: ServerInfo,
    pub endpoint_id: String,
    pub relay: Option<String>,
    /// A count rather than the addresses themselves. A pairing screen says
    /// "2 direct addresses"; the addresses would be noise on a phone.
    pub direct_addr_count: u16,
    pub code_length: u8,
    pub code_format: CodeFormat,
    #[ts(type = "number")]
    pub expires_in_ms: u32,
}
