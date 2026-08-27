use tethera_common::protocol::error::WireError;
use tethera_common::protocol::handshake::{DeviceRecord, ServerInfo};
use tethera_common::protocol::response::{ConversationPreview, Describe, Page};
use tethera_common::protocol::terminal::{
    AttachSpec, RowUpdate, Style, TerminalFrame, TerminalInput,
};
use tethera_common::protocol::transfer::{FetchHead, PutReady, PutResult, PutSpec};
use tethera_common::protocol::watch::WatchEvent;
use tethera_common::structs::agent::AgentProfile;
use tethera_common::structs::asset::{AssetCard, AssetScope};
use tethera_common::structs::conversation::{Conversation, ConversationFilter};
use tethera_common::structs::ids::{
    AssetId, ConversationId, PaneId, ProfileId, QuestionId, TabId, WorkspaceId,
};
use tethera_common::structs::primitives::{Cursor, Fingerprint};
use tethera_common::structs::terminal::{Pane, SplitDirection, Tab, TabLayout, Workspace};
use tethera_common::structs::transcript::{Answer, Turn};
use tokio::sync::broadcast;

/// Every rank of the tree in one value, because that is how a watch opens.
#[derive(Debug, Clone)]
pub struct TreeSnapshot {
    pub workspaces: Vec<Workspace>,
    pub tabs: Vec<Tab>,
    pub panes: Vec<Pane>,
    pub conversations: Vec<Conversation>,
    /// One per tab whose geometry the backend would vouch for. A tab that is
    /// absent has none, and the client draws no map for it.
    pub layouts: Vec<TabLayout>,
}

impl TreeSnapshot {
    pub fn layout_of(&self, tab: &TabId) -> Option<&TabLayout> {
        self.layouts.iter().find(|layout| &layout.tab == tab)
    }
}

/// What an enrolment attempt was told about the machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnrollOffer {
    pub server: ServerInfo,
    pub code_length: u8,
    pub expires_in_ms: u32,
}

/// Whether this endpoint id may open a session, and on what terms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Enrolment {
    /// Known and permitted.
    Known(DeviceRecord),
    /// Known and revoked. Distinct from unknown: a revoked device must not be
    /// able to re-enrol by pretending it was never here.
    Revoked,
    /// Not known to this machine.
    Unknown,
}

/// One attached pane: frames out, input in.
///
/// The emulator that produces these frames is not part of this plan. A fake
/// produces them directly; a follow-up puts a vte parser behind the same trait.
pub trait TerminalSession: Send {
    fn next_frame(&mut self) -> impl std::future::Future<Output = Option<TerminalFrame>> + Send;

    fn send_input(
        &mut self,
        input: TerminalInput,
    ) -> impl std::future::Future<Output = Result<(), WireError>> + Send;
}

/// One page of scrollback: the style table, the rows, and the source's own answer
/// about whether more exists.
pub type ScrollbackPage = (Vec<Style>, Vec<RowUpdate>, Option<u32>, bool);

pub trait MachinePort: Send + Sync {
    fn describe(&self) -> impl std::future::Future<Output = Describe> + Send;

    fn server_info(&self) -> ServerInfo;

    fn agent_profiles(&self) -> impl std::future::Future<Output = Vec<AgentProfile>> + Send;

    fn recent_cwds(&self, limit: u16) -> impl std::future::Future<Output = Vec<String>> + Send;

    fn tree(&self) -> impl std::future::Future<Output = Result<TreeSnapshot, WireError>> + Send;

    /// Changes to any rank of the tree, for a machine watch.
    fn tree_events(&self) -> broadcast::Receiver<WatchEvent>;

    /// Whether this endpoint id may open a session.
    ///
    /// A plain lookup, so the handshake decision below it stays a pure function
    /// of its inputs and is testable without a database.
    fn enrolment(
        &self,
        endpoint_id: &str,
    ) -> impl std::future::Future<Output = Enrolment> + Send;

    /// `Some` when a human has opened a pairing window on the machine.
    ///
    /// An unknown endpoint id with no window open is refused immediately and
    /// nothing is displayed, which is what stops a stranger's endpoint id from
    /// making a code appear on your screen.
    fn pairing_window(&self) -> impl std::future::Future<Output = Option<EnrollOffer>> + Send;

    /// Compares a typed code and enrols on success.
    fn redeem_code(
        &self,
        endpoint_id: &str,
        code: &str,
        device_name: &str,
    ) -> impl std::future::Future<Output = Result<DeviceRecord, u8>> + Send;

    /// Drop this endpoint id from the allow-list.
    fn revoke(&self, endpoint_id: &str)
        -> impl std::future::Future<Output = Result<(), WireError>> + Send;
}

pub trait ConversationPort: Send + Sync {
    fn list(
        &self,
        filter: ConversationFilter,
        before: Option<Cursor>,
        limit: u16,
    ) -> impl std::future::Future<Output = Page<Conversation>> + Send;

    fn get(
        &self,
        id: &ConversationId,
    ) -> impl std::future::Future<Output = Result<Conversation, WireError>> + Send;

    fn transcript(
        &self,
        id: &ConversationId,
        before: Option<Cursor>,
        limit: u16,
    ) -> impl std::future::Future<Output = Result<Page<Turn>, WireError>> + Send;

    /// Live turns and question changes for one conversation, plus the cursor the
    /// stream actually starts at. A `from` later than the requested `after` tells
    /// the client its cursor predates the earliest surviving record.
    fn subscribe(
        &self,
        id: &ConversationId,
        after: Option<Cursor>,
    ) -> impl std::future::Future<
        Output = Result<(Cursor, broadcast::Receiver<WatchEvent>), WireError>,
    > + Send;

    fn preview(
        &self,
        profile: &ProfileId,
        cwd: &str,
        workspace: Option<&WorkspaceId>,
    ) -> impl std::future::Future<Output = Result<ConversationPreview, WireError>> + Send;

    fn start(
        &self,
        profile: &ProfileId,
        cwd: &str,
        prompt: Option<&str>,
        attachments: &[AssetId],
    ) -> impl std::future::Future<Output = Result<Conversation, WireError>> + Send;

    fn resume(
        &self,
        id: &ConversationId,
        cwd: Option<&str>,
    ) -> impl std::future::Future<Output = Result<Conversation, WireError>> + Send;

    fn send_prompt(
        &self,
        id: &ConversationId,
        text: &str,
        attachments: &[AssetId],
    ) -> impl std::future::Future<Output = Result<(), WireError>> + Send;

    /// Stop what the agent is doing. The conversation survives.
    fn interrupt(
        &self,
        id: &ConversationId,
    ) -> impl std::future::Future<Output = Result<(), WireError>> + Send;

    /// End the agent process. History survives.
    fn stop(
        &self,
        id: &ConversationId,
    ) -> impl std::future::Future<Output = Result<(), WireError>> + Send;

    /// Refuses `Stale` when the fingerprint no longer matches, rather than
    /// answering a different question blind.
    fn answer(
        &self,
        id: &ConversationId,
        question: &QuestionId,
        fingerprint: &Fingerprint,
        answers: &[Answer],
    ) -> impl std::future::Future<Output = Result<(), WireError>> + Send;
}

pub trait TerminalPort: Send + Sync {
    type Session: TerminalSession;

    fn list_tabs(
        &self,
        workspace: &WorkspaceId,
    ) -> impl std::future::Future<Output = Result<Vec<Tab>, WireError>> + Send;

    fn list_panes(
        &self,
        tab: &TabId,
    ) -> impl std::future::Future<Output = Result<Vec<Pane>, WireError>> + Send;

    /// Where this tab's panes sit, in cells.
    fn layout(
        &self,
        tab: &TabId,
    ) -> impl std::future::Future<Output = Result<TabLayout, WireError>> + Send;

    /// Move the machine's own focus to this tab.
    fn focus_tab(
        &self,
        tab: &TabId,
    ) -> impl std::future::Future<Output = Result<(), WireError>> + Send;

    /// Creates a new tab. Geometry is decided here and is stable for the pane's
    /// life; there is no resize in this protocol.
    fn open(
        &self,
        workspace: Option<&WorkspaceId>,
        cwd: Option<&str>,
    ) -> impl std::future::Future<Output = Result<Pane, WireError>> + Send;

    fn split(
        &self,
        pane: &PaneId,
        direction: SplitDirection,
    ) -> impl std::future::Future<Output = Result<Pane, WireError>> + Send;

    fn close(
        &self,
        pane: &PaneId,
    ) -> impl std::future::Future<Output = Result<(), WireError>> + Send;

    /// Takes the whole spec rather than a pane id.
    ///
    /// The view and the viewport decide what the backend is asked for and how
    /// its output is laid out, and both are known only to the client that is
    /// about to draw it.
    fn attach(
        &self,
        spec: &AttachSpec,
    ) -> impl std::future::Future<Output = Result<Self::Session, WireError>> + Send;

    fn scrollback(
        &self,
        pane: &PaneId,
        before_line: Option<u32>,
        limit: u16,
    ) -> impl std::future::Future<Output = Result<ScrollbackPage, WireError>> + Send;
}

pub trait AssetPort: Send + Sync {
    /// What a fetch hands back to be streamed.
    ///
    /// An associated type rather than a `Vec<u8>`, so a download is read and
    /// written in chunks instead of loaded whole. A machine serving a file it
    /// first had to hold in memory is bounded by the largest file it will ever
    /// send, and that bound has nothing to do with how big a file is worth
    /// serving.
    type Body: std::io::Read + Send + 'static;

    fn list(
        &self,
        scope: &AssetScope,
        before: Option<Cursor>,
        limit: u16,
    ) -> impl std::future::Future<Output = Result<Page<AssetCard>, WireError>> + Send;

    /// The head frame plus a reader positioned at `offset`.
    ///
    /// `FetchHead.len` and `FetchHead.sha256` describe the **whole** asset, not
    /// what is about to be sent: a client checks its finished download against
    /// that digest, and a digest of the range would pass a truncated file.
    fn fetch(
        &self,
        asset: &AssetId,
        offset: u64,
    ) -> impl std::future::Future<Output = Result<(FetchHead, Self::Body), WireError>> + Send;

    /// The offset the server wants, which is authoritative over the client's
    /// proposal because only the server knows how much of a previous attempt
    /// reached disk.
    fn put_ready(
        &self,
        spec: &PutSpec,
    ) -> impl std::future::Future<Output = Result<PutReady, WireError>> + Send;

    /// Verifies the digest before issuing an id; a mismatch is a `Backend` error
    /// rather than a silently corrupt attachment.
    fn put_finish(
        &self,
        spec: &PutSpec,
        body: &[u8],
    ) -> impl std::future::Future<Output = Result<PutResult, WireError>> + Send;
}

/// The four ports one machine offers.
pub trait Ports: Send + Sync + 'static {
    type Machine: MachinePort;
    type Conversations: ConversationPort;
    type Terminals: TerminalPort;
    type Assets: AssetPort;

    fn machine(&self) -> &Self::Machine;
    fn conversations(&self) -> &Self::Conversations;
    fn terminals(&self) -> &Self::Terminals;
    fn assets(&self) -> &Self::Assets;
}
