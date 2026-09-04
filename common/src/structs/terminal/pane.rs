use super::Size;
use crate::structs::ids::{ConversationId, PaneId, ProfileId, TabId, WorkspaceId};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// One pane, normalised.
///
/// No field here is a herdr field. This is tethera's own model, and a backend
/// adapter maps a terminal multiplexer onto it, so tmux is another adapter
/// rather than another protocol.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct Pane {
    pub id: PaneId,
    pub tab_id: TabId,
    pub workspace_id: WorkspaceId,
    pub label: String,
    pub title: Option<String>,
    pub cwd: Option<String>,
    pub size: Size,
    pub focused: bool,
    pub foreground_command: Option<String>,
    pub conversation: Option<ConversationId>,
    /// The agent running in this pane, whether or not it said which session.
    ///
    /// Separate from conversation on purpose. A backend routinely reports that
    /// an agent is running in a pane while having no session identity for it,
    /// and one nullable field cannot carry both facts: without this, a live
    /// agent nobody can name is indistinguishable from an empty shell, and a
    /// client offers to resume a conversation that may already be running.
    ///
    /// The pair leaves exactly three states reachable, which is the point:
    /// neither set is an empty pane, both set is running and identified, and
    /// this one alone is running but unnamed.
    pub agent: Option<ProfileId>,
    /// Whether this pane's output can be streamed to a client.
    ///
    /// True when a shim is relaying it. False for a pane that started before the
    /// shim was in its shell — herdr still owns it and it is still on screen at
    /// the desk, but nothing here can read it, so a client must draw it as
    /// unavailable rather than let somebody tap into a refusal.
    ///
    /// Per pane rather than per machine, which is why it lives here and not in
    /// the capability set: one machine has both kinds at once.
    pub streamed: bool,
}

impl Pane {
    pub fn new(
        id: PaneId,
        tab_id: TabId,
        workspace_id: WorkspaceId,
        label: String,
        size: Size,
    ) -> Self {
        Self {
            id,
            tab_id,
            workspace_id,
            label,
            title: None,
            cwd: None,
            size,
            focused: false,
            foreground_command: None,
            conversation: None,
            agent: None,
            // Set by the port, which is the only layer that knows whether a shim
            // has adopted this pane. A backend builds panes without ever seeing
            // the registry.
            streamed: false,
        }
    }
}
