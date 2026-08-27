use crate::protocol::capability::CapabilitySet;
use crate::protocol::WireVersion;
use crate::structs::ids::{DeviceId, RequestId, ServerId};
use crate::structs::primitives::Timestamp;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub enum Platform {
    Ios,
    Android,
    Desktop,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct ClientInfo {
    pub app_version: String,
    pub platform: Platform,
    /// A stable per-install identifier. Without it, every re-pair leaves a stale
    /// device record instead of replacing the earlier one.
    pub install_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub enum Intent {
    /// An endpoint id the server already knows.
    Session,
    /// An endpoint id the server does not know, offering to enroll. Refused
    /// unless a human has opened a pairing window on the machine.
    Enroll,
}

/// The mandatory first frame on a connection.
///
/// QUIC TLS has already proved the peer's endpoint id by the time this arrives,
/// so nothing here is a credential. There are no bearer tokens, no transport
/// keys and no sealed envelopes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct ClientHello {
    pub versions: Vec<WireVersion>,
    pub client: ClientInfo,
    pub intent: Intent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct ServerInfo {
    pub id: ServerId,
    pub label: String,
    pub app_version: String,
    /// The client lists machines by what they are, and cannot know that from an
    /// endpoint id.
    pub os: String,
    pub arch: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct DeviceRecord {
    pub id: DeviceId,
    pub name: String,
    pub paired_at: Timestamp,
}

/// What a person is about to type off a screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub enum CodeFormat {
    Digits,
    Alphanumeric,
}

/// Why a connection was refused.
///
/// Deliberately narrower than the general `WireError`: a connection can fail for
/// only a few reasons, and naming them exhaustively is what lets a client
/// explain the failure instead of showing a string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub enum RefuseReason {
    /// This endpoint id is not enrolled and no pairing window is open.
    NotEnrolled,
    /// This endpoint id offered to enroll, but no window is open.
    PairingWindowClosed,
    /// No version both sides speak. A refusal, not a fallback.
    NoCommonVersion,
    /// This endpoint id was enrolled and has been revoked.
    Revoked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub enum ServerHello {
    Session {
        version: WireVersion,
        server: ServerInfo,
        /// Asked, never guessed. A version string does not answer this: a
        /// feature can be absent on a machine that is otherwise current.
        capabilities: CapabilitySet,
        device: DeviceRecord,
    },
    /// A code is now displayed on the machine. The client sends `EnrollCode` on
    /// this same stream.
    EnrollPending {
        request_id: RequestId,
        #[ts(type = "number")]
        expires_in_ms: u32,
        /// Who answered. TLS has already proved this endpoint id, so this is
        /// server-attested rather than copied off a QR, and the client can name
        /// the machine it is about to pair before anything is typed.
        server: ServerInfo,
        /// How many cells to draw, and what a person is about to type. A client
        /// cannot lay out a code entry without them.
        code_length: u8,
        code_format: CodeFormat,
    },
    Refuse(RefuseReason),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct EnrollCode {
    pub request_id: RequestId,
    pub code: String,
    pub device_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub enum EnrollResult {
    Accepted {
        device: DeviceRecord,
        version: WireVersion,
        server: ServerInfo,
        capabilities: CapabilitySet,
    },
    Refused {
        reason: RefuseReason,
        attempts_left: u8,
    },
}

/// The form a pairing code is compared in.
///
/// The code is read off a screen and typed on a phone, which adds spaces and
/// changes case. Comparing it raw fails a person who typed it correctly.
pub struct Handshake;

impl Handshake {
    pub fn normalize_code(code: &str) -> String {
        code.trim().to_uppercase()
    }
}
