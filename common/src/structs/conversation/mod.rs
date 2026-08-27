mod stats;

pub use stats::AgentStats;

use crate::structs::agent::AgentStatus;
use crate::structs::ids::{ConversationId, PaneId, ProfileId, WorkspaceId};
use crate::structs::primitives::Timestamp;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub enum ConversationFilter {
    /// Everything the machine's transcript index knows about.
    All,
    /// Only conversations currently bound to a pane.
    Live,
    /// Only conversations waiting on a person.
    Blocked,
}

/// One agent conversation.
///
/// First-class and independent of any pane. `binding` is `None` when nothing is
/// running: history still reads, and the client offers to resume, which is a
/// separate and deliberate act because it starts a process on the machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct Conversation {
    pub id: ConversationId,
    pub profile: ProfileId,
    /// Display only. The client never branches on which agent this is.
    pub profile_label: String,
    pub title: Option<String>,
    /// One line of the most recent meaningful text: the pending question's
    /// prompt when blocked, otherwise the agent's last words. The server derives
    /// it, because deciding what is meaningful is the same judgement as the
    /// noise filter and belongs in one place. Without it, drawing a home screen
    /// across three machines costs a transcript request per conversation before
    /// anything appears.
    pub preview: Option<String>,
    pub cwd: String,
    /// `None` once the workspace is gone. Walking up through `binding` cannot
    /// replace this: an unbound conversation has no pane, which is exactly the
    /// rebooted-machine case where the grouping matters most.
    pub workspace: Option<WorkspaceId>,
    pub started_at: Timestamp,
    pub last_active: Option<Timestamp>,
    #[ts(type = "number | null")]
    pub turn_count: Option<u32>,
    pub status: AgentStatus,
    /// Whether this conversation has a readable transcript at all. The client
    /// has to know before it opens a screen, so it can offer the terminal
    /// instead of an empty conversation.
    pub has_transcript: bool,
    pub binding: Option<PaneId>,
    /// Whether this machine will actually start this conversation again.
    ///
    /// False when it cannot rule out that the conversation is **already
    /// running** - a pane in the same directory holds an agent that never
    /// announced its session, so nothing on this machine can say whether it is
    /// this one. Resuming anyway would put a second process on one set of
    /// records, and two agents appending to one history corrupt the surface
    /// every other screen reads from.
    ///
    /// Sent rather than left to the refusal, so a client can say why instead of
    /// drawing a control that fails on press. An unbound conversation that is
    /// merely finished is , which is the ordinary case.
    pub resumable: bool,
}
