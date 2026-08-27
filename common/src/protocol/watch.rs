use crate::structs::conversation::{AgentStats, Conversation};
use crate::structs::ids::{ConversationId, PaneId, QuestionId, TabId, WorkspaceId};
use crate::structs::primitives::Cursor;
use crate::structs::terminal::{Pane, Tab, Workspace};
use crate::structs::transcript::{Question, Turn};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub enum WatchSpec {
    /// Conversations, panes and statuses on this machine.
    Machine,
    /// One conversation's live tail, resuming after a cursor the client holds.
    Conversation {
        id: ConversationId,
        after: Option<Cursor>,
    },
}

/// The server's first frame on a watch stream. Always sent, always exactly one.
///
/// There are no ring buffers and no durable event log behind this. Every
/// subscription is backed by a source the server can re-read - the transcript is
/// a file, the grid is live in the emulator, the pane list is live in the
/// backend - so a reconnecting client re-subscribes and gets a fresh snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub enum WatchOpen {
    /// The whole tree, every rank of it, in one frame. The client draws
    /// machines, workspaces, tabs and conversations together before anything is
    /// tapped, so sending less means a request per rank per machine before the
    /// first screen appears.
    Machine {
        workspaces: Vec<Workspace>,
        tabs: Vec<Tab>,
        panes: Vec<Pane>,
        conversations: Vec<Conversation>,
    },
    Conversation {
        conversation: Conversation,
        /// Where the stream actually starts, which is not always the `after` the
        /// client asked for. Later than the request means the client's cursor
        /// predates the earliest surviving record, and it should refetch the gap
        /// rather than render a hole it cannot see.
        from: Cursor,
    },
}

/// The client's side of a watch stream. A reset is equally valid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub enum WatchControl {
    Close,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub enum WatchEvent {
    WorkspaceChanged(Workspace),
    WorkspaceRemoved(WorkspaceId),
    TabChanged(Tab),
    TabRemoved(TabId),
    ConversationChanged(Conversation),
    ConversationRemoved(ConversationId),
    PaneChanged(Pane),
    PaneRemoved(PaneId),
    /// Conversation watch only.
    Turn(Turn),
    /// Conversation watch only. The same `Question` the transcript carries, so
    /// the client has one surface for both and cannot tell which detector found
    /// it.
    Blocked {
        question: Question,
    },
    Unblocked {
        question: QuestionId,
    },
    /// What the agent is doing right now, in figures.
    ///
    /// Conversation watch only, and sent when the figures change rather than on
    /// a clock: `AgentStats.turn_started_at` is a start, so a client ticks its
    /// own elapsed count and this does not have to arrive once a second to move
    /// a number the client could move itself.
    ///
    /// Last, and it has to stay last. postcard encodes a variant by its index,
    /// so putting this anywhere else renumbers every variant after it and a
    /// client already shipped decodes one event as another.
    Stats(AgentStats),
}
