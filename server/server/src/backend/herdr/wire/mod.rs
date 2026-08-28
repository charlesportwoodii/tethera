//! herdr's own JSON, one type per file.
//!
//! Field lists come from `herdr api schema --json` rather than from a captured
//! sample, so a field the schema marks optional is `Option` here even when
//! every sample carries it. Nothing declares `deny_unknown_fields`: herdr's
//! schema grows additively, and refusing a field this backend ignores would
//! break on a release that changed nothing we read.
//!
//! `result` is a tagged union. Every variant puts its payload under its own key
//! beside `type`, and the two that nest it — `session_snapshot` and
//! `pane_process_info` — have a body type of their own for that reason.

mod agent_body;
mod agent_info;
mod agent_session;
mod created;
mod envelope;
mod failure;
mod foreground_process;
mod layout_pane;
mod layout_split;
mod pane_body;
mod pane_info;
mod pane_layout;
mod pane_scroll_info;
mod process_info;
mod process_info_body;
mod rect;
mod snapshot_body;
mod tab_info;
mod workspace_info;
mod worktree_info;

pub use agent_body::AgentBody;
pub use agent_info::AgentInfo;
pub use agent_session::{AgentSession, AgentSessionKind};
pub use created::Created;
pub use envelope::Envelope;
pub use failure::Failure;
pub use foreground_process::ForegroundProcess;
pub use layout_pane::LayoutPane;
pub use layout_split::LayoutSplit;
pub use pane_body::PaneBody;
pub use pane_info::PaneInfo;
pub use pane_layout::PaneLayout;
pub use pane_scroll_info::PaneScrollInfo;
pub use process_info::ProcessInfo;
pub use process_info_body::ProcessInfoBody;
pub use rect::Rect;
pub use snapshot_body::SnapshotBody;
pub use tab_info::TabInfo;
pub use workspace_info::WorkspaceInfo;
pub use worktree_info::WorktreeInfo;

use serde::Deserialize;

/// The whole session in one answer.
///
/// `herdr api snapshot` carries every rank of the tree plus the layouts, so a
/// tree render is one subprocess call. Asking `workspace list`, then `tab list`
/// per workspace, then `pane list` per tab would be the request storm the
/// protocol's denormalised fields exist to prevent, moved one layer down where
/// the client cannot see it.
#[derive(Debug, Clone, Deserialize)]
pub struct Snapshot {
    pub version: String,
    pub protocol: u32,
    pub workspaces: Vec<WorkspaceInfo>,
    pub tabs: Vec<TabInfo>,
    pub panes: Vec<PaneInfo>,
    #[serde(default)]
    pub layouts: Vec<PaneLayout>,
    /// The subset of `panes` that has an agent, with the agent lifecycle fields
    /// added. Nothing here reads it: a pane's own record already carries
    /// `agent`, `terminal_title` and `agent_session`, and the only fields
    /// unique to this array are `interactive_ready`, `launch_pending`,
    /// `screen_detection_skipped` and `state_change_seq`, which are the
    /// conversation surface's business rather than the tree's.
    #[serde(default)]
    pub agents: Vec<AgentInfo>,
    #[serde(default)]
    pub focused_workspace_id: Option<String>,
    #[serde(default)]
    pub focused_tab_id: Option<String>,
    #[serde(default)]
    pub focused_pane_id: Option<String>,
}

impl Snapshot {
    /// The protocols this backend's field expectations were written against.
    ///
    /// Both are in the field at once: a herdr preview still speaks 19 where a
    /// current release speaks 20, and the two differ in nothing read here.
    ///
    /// Recorded rather than enforced. Every field the mapping reads is
    /// required, so a removal already fails loudly at the parse; refusing a
    /// whole session over a version bump that only added a field would take the
    /// product down for no gain.
    pub const KNOWN_PROTOCOLS: &'static [u32] = &[19, 20];

    pub fn speaks_known_protocol(&self) -> bool {
        Self::KNOWN_PROTOCOLS.contains(&self.protocol)
    }

    pub fn layout_of_tab(&self, tab_id: &str) -> Option<&PaneLayout> {
        self.layouts.iter().find(|layout| layout.tab_id == tab_id)
    }

    /// The pane whose facts a tab row draws.
    ///
    /// Layout order first, because that is the order a person reading the
    /// screen would call first-to-last. `panes` order is the fallback for a tab
    /// no layout describes.
    pub fn primary_pane_of_tab(&self, tab_id: &str) -> Option<&PaneInfo> {
        let by_layout = self
            .layout_of_tab(tab_id)
            .and_then(|layout| layout.panes.first())
            .and_then(|first| self.pane(&first.pane_id));

        by_layout.or_else(|| self.panes.iter().find(|pane| pane.tab_id == tab_id))
    }

    pub fn pane(&self, pane_id: &str) -> Option<&PaneInfo> {
        self.panes.iter().find(|pane| pane.pane_id == pane_id)
    }
}
