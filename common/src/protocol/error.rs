use crate::protocol::capability::CapabilityId;
use crate::protocol::WireVersion;
use crate::structs::ids::{ConversationId, PaneId};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub enum EntityKind {
    Server,
    Session,
    Workspace,
    Tab,
    Pane,
    Conversation,
    Asset,
    Question,
    Device,
}

/// A failure the protocol reports, as opposed to `TransportError`, which is a
/// failure turning a frame into bytes or moving it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub enum WireError {
    NotEnrolled,
    PairingWindowClosed,
    BadCode {
        attempts_left: u8,
    },
    /// Names what the server speaks, so a client can tell a person to update
    /// rather than showing "connection failed".
    NoCommonVersion {
        server_supports: Vec<WireVersion>,
    },
    NotFound {
        kind: EntityKind,
    },
    /// The fingerprint no longer matches: the pane has moved on to a different
    /// question. Refusing is the point - answering blind would answer the wrong
    /// question.
    Stale,
    /// The link to the terminal backend is one serialised connection and a call
    /// is holding it. A real, reportable state rather than a silent stall.
    Busy,
    Unsupported {
        capability: CapabilityId,
    },
    TooLarge {
        #[ts(type = "number")]
        size: u64,
        #[ts(type = "number")]
        limit: u64,
    },
    Backend {
        message: String,
    },
    /// The agent started, and has begun no session of its own.
    ///
    /// Not a failed start and not a `Backend` failure: the agent is running in
    /// `pane`, and something at the machine is holding it before its first
    /// record — a directory it has not been trusted with, a sign-in, an
    /// onboarding screen. Nothing on the wire can tell those apart, and nothing
    /// needs to: what a client shows is that it started and is waiting at the
    /// machine, and `pane` is where to look.
    ///
    /// Its own variant rather than a sentence inside `Backend`, because a client
    /// has to distinguish this from a real failure and matching on prose would
    /// make that classification turn on wording no test guards.
    AwaitingAgent {
        pane: PaneId,
    },
    /// The conversation has no agent running, so there is nothing to type at.
    ///
    /// Distinct from `NotFound`: the records are here and readable, and the
    /// conversation can be put back in front of a person by resuming it. A
    /// client that could not tell the two apart would offer "gone" where it
    /// should offer "pick it up again".
    NotRunning {
        conversation: ConversationId,
    },
}
