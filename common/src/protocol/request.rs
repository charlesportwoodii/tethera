use crate::protocol::push::{NotifyPolicy, PushPlatform};
use crate::structs::asset::AssetScope;
use crate::structs::conversation::ConversationFilter;
use crate::structs::ids::{
    AssetId, ConversationId, PaneId, ProfileId, QuestionId, TabId, WorkspaceId,
};
use crate::structs::primitives::{Cursor, Fingerprint};
use crate::structs::terminal::SplitDirection;
use crate::structs::transcript::Answer;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// One request, one stream.
///
/// There is no request id: QUIC is the multiplexer, so the stream is the
/// correlation. Cancellation is resetting the stream, which both ends observe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub enum Request {
    // machine
    Describe,
    ListAgentProfiles,
    RecentCwds {
        limit: u16,
    },

    // conversations
    ListConversations {
        filter: ConversationFilter,
        before: Option<Cursor>,
        limit: u16,
    },
    GetConversation {
        conversation: ConversationId,
    },
    StartConversation {
        profile: ProfileId,
        cwd: String,
        prompt: Option<String>,
        attachments: Vec<AssetId>,
    },
    ResumeConversation {
        conversation: ConversationId,
        cwd: Option<String>,
    },
    /// What starting this would create, without creating it. The alternative is
    /// the client guessing a name the server generates, which is the inference
    /// this protocol exists to avoid.
    PreviewConversation {
        profile: ProfileId,
        cwd: String,
        workspace: Option<WorkspaceId>,
    },
    SendPrompt {
        conversation: ConversationId,
        text: String,
        attachments: Vec<AssetId>,
    },
    /// Stop what the agent is doing. The conversation survives.
    Interrupt {
        conversation: ConversationId,
    },
    /// End the agent process. History survives and it can be resumed. A
    /// different act from `Interrupt`, with a different consequence, so it is a
    /// different request rather than a flag on one.
    StopConversation {
        conversation: ConversationId,
    },
    /// One answer per question in the set, in the set's own order.
    ///
    /// The whole set at once, because the agent stays blocked until it has every
    /// answer and its picker is one piece of screen state. A trickle would move
    /// that picker halfway between requests.
    AnswerQuestion {
        conversation: ConversationId,
        question: QuestionId,
        fingerprint: Fingerprint,
        answers: Vec<Answer>,
    },
    /// `before: None` asks for the most recent page.
    Transcript {
        conversation: ConversationId,
        before: Option<Cursor>,
        limit: u16,
    },

    // terminal structure. Creating and destroying only: a client never moves,
    // resizes or focuses a pane.
    ListWorkspaces,
    ListTabs {
        workspace: WorkspaceId,
    },
    ListPanes {
        tab: TabId,
    },
    /// Creates a new tab, in the named workspace or in a new one. A second pane
    /// inside an existing tab is `SplitPane`, the only operation that needs a
    /// direction.
    OpenTerminal {
        workspace: Option<WorkspaceId>,
        cwd: Option<String>,
    },
    SplitPane {
        pane: PaneId,
        direction: SplitDirection,
    },
    ClosePane {
        pane: PaneId,
    },
    TerminalScrollback {
        pane: PaneId,
        #[ts(type = "number | null")]
        before_line: Option<u32>,
        limit: u16,
    },

    // files
    ListAssets {
        scope: AssetScope,
        before: Option<Cursor>,
        limit: u16,
    },

    // push
    RegisterPushToken {
        platform: PushPlatform,
        token: String,
    },
    RevokePushToken {
        token: String,
    },
    /// Per machine. Each machine holds its own policy and calls FCM itself, so a
    /// client offering one switch sends this to every machine it is paired to,
    /// and has to say what happened when one of them is unreachable.
    SetNotifyPolicy {
        policy: NotifyPolicy,
    },

    // device
    /// Drop this device's own endpoint id from the machine's allow-list. The
    /// connection closes as it completes and the next is refused. There is no
    /// token to forget: the endpoint id is the identity, so revoking removes the
    /// identity itself.
    RevokeThisDevice,
}
