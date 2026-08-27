use crate::protocol::capability::CapabilitySet;
use crate::protocol::error::WireError;
use crate::protocol::handshake::ServerInfo;
use crate::protocol::terminal::{RowUpdate, Style};
use crate::structs::agent::AgentProfile;
use crate::structs::asset::AssetCard;
use crate::structs::conversation::Conversation;
use crate::structs::primitives::Cursor;
use crate::structs::terminal::{Pane, Tab, Workspace};
use crate::structs::transcript::Turn;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The bounds a machine enforces, so a client can size its own requests rather
/// than discovering a limit by hitting it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct Limits {
    #[ts(type = "number")]
    pub max_control_frame: u32,
    pub max_streams: u16,
    pub transcript_page: u16,
    pub scrollback_page: u16,
    /// `None` when the machine states no upload bound.
    #[ts(type = "number | null")]
    pub max_upload: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct Describe {
    pub server: ServerInfo,
    pub capabilities: CapabilitySet,
    pub limits: Limits,
}

/// One page of anything cursor-paged.
///
/// `has_earlier` is the source's own answer, and a client believes it over any
/// heuristic of its own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct Page<T> {
    pub items: Vec<T>,
    pub next_before: Option<Cursor>,
    pub has_earlier: bool,
}

/// What a create would make, without making it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct ConversationPreview {
    /// "tethera-4".
    pub workspace_label: String,
    /// "claude".
    pub tab_label: String,
    pub creates_workspace: bool,
    /// Whether this profile in this directory will have a readable transcript.
    /// Repeated from `AgentProfile` because the answer can depend on the
    /// directory, not only on the profile.
    pub will_have_transcript: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub enum Payload {
    Describe(Describe),
    AgentProfiles(Vec<AgentProfile>),
    RecentCwds(Vec<String>),
    Conversations(Page<Conversation>),
    Conversation(Conversation),
    ConversationPreview(ConversationPreview),
    /// Oldest first.
    Transcript(Page<Turn>),
    Workspaces(Vec<Workspace>),
    Tabs(Vec<Tab>),
    Panes(Vec<Pane>),
    Pane(Pane),
    Scrollback {
        styles: Vec<Style>,
        rows: Vec<RowUpdate>,
        #[ts(type = "number | null")]
        next_before_line: Option<u32>,
        has_earlier: bool,
    },
    Assets(Page<AssetCard>),
    /// Nothing to return, and it worked.
    Ack,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub enum ProgressStage {
    Accepted,
    WaitingOnBackend,
    StartingAgent,
    Ready,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct Progress {
    pub stage: ProgressStage,
    pub detail: Option<String>,
}

/// What a server writes on an RPC stream: zero or more `Progress`, then exactly
/// one of `Ok` or `Err`.
///
/// Progress is what makes a slow operation visible. The predecessor's symptom
/// was "a new workspace appears but the agent never starts", intermittently,
/// with nothing logged at either end. Here the operation proves it is alive, and
/// cancellation is a stream reset both ends observe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub enum Response {
    Progress(Progress),
    Ok(Payload),
    Err(WireError),
}

impl Response {
    /// Whether this frame ends the stream.
    ///
    /// A handler writes zero or more frames for which this is false, then
    /// exactly one for which it is true. Stated here rather than left to each
    /// handler, because a client that saw two terminal frames - or none - could
    /// not tell a finished call from a stalled one.
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Ok(_) | Self::Err(_))
    }
}
