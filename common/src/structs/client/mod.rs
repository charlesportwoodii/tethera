mod attached;
mod begin;
mod controls;
mod download;
mod found;
mod live;
mod outcome;
mod preferences;
mod preview;
mod row;
mod send;
mod start;

pub use attached::Attached;
pub use begin::BeginOutcome;
pub use controls::ConversationControls;
pub use download::{DownloadProgress, DownloadState};
pub use found::FoundServer;
pub use live::ConversationEvent;
pub use outcome::PairOutcome;
pub use preferences::Preferences;
pub use preview::AssetPreview;
pub use row::ServerRow;
pub use send::SendOutcome;
pub use start::StartOutcome;

use crate::protocol::capability::CapabilitySet;
use crate::protocol::handshake::{DeviceRecord, ServerInfo};
use crate::structs::conversation::Conversation;
use crate::structs::primitives::Timestamp;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// One machine a client remembers.
///
/// Every field is server-attested at the last handshake rather than copied from
/// the pairing offer. A QR is a claim by whoever printed it; the handshake is
/// proved by the peer's TLS certificate, because the endpoint id is the public
/// key.
///
/// Holds no `Link`. A measurement written to disk and read back at launch would
/// paint a live dot for a machine that is not answering, so the measured value
/// lives on `ServerRow` and is produced fresh by each sweep.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct ServerEntry {
    pub server: ServerInfo,
    pub endpoint_id: String,
    pub relay: Option<String>,
    pub direct_addrs: Vec<String>,
    /// What this machine calls this client, and when it accepted it.
    pub device: DeviceRecord,
    pub capabilities: CapabilitySet,
    pub last_seen_at: Option<Timestamp>,
    /// What was running the last time this machine answered.
    ///
    /// Persisted, unlike the link. A machine that is not answering still shows
    /// what it was doing when it went quiet, dated by `last_seen_at` - which is
    /// the one useful fact on an otherwise empty screen, and the starting point
    /// for resuming a conversation that no longer has a pane attached.
    ///
    /// Defaulted so a book written before this field existed still loads. A
    /// missing field is not corruption, and refusing to open over one would lock
    /// somebody out of machines they are already paired to.
    #[serde(default)]
    pub conversations: Vec<Conversation>,
}
