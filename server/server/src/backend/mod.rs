pub mod error;
pub mod herdr;
pub mod tree;

pub use error::BackendError;
pub use herdr::HerdrBackend;
pub use tree::BackendTree;

use crate::terminal::{PaneRegistry, PtyBackend};
use std::sync::Arc;
use tethera_common::protocol::terminal::{Key, Mods};
use tethera_common::structs::agent::AgentSpawn;
use tethera_common::structs::ids::{ConversationId, PaneId, TabId, WorkspaceId};
use tethera_common::structs::terminal::{Pane, Size, SplitDirection, Tab, TabLayout, Workspace};
use tethera_common::traits::TerminalBackendTrait;

pub enum TerminalBackend {
    Herdr(HerdrBackend),
    /// Ptys this process owns. The only variant that can be attached to, because
    /// herdr publishes no per-pane byte stream.
    Pty(PtyBackend),
}

impl TerminalBackend {
    pub fn herdr(binary: String, default_size: Size) -> Self {
        Self::Herdr(HerdrBackend::new(binary, default_size))
    }

    pub fn pty(registry: Arc<PaneRegistry>, default_size: Size, shell: String) -> Self {
        Self::Pty(PtyBackend::new(registry, default_size, shell))
    }

    /// Whether a pane from this backend can be emulated.
    ///
    /// What `terminal_attach` and `terminal_input` are advertised on, so a
    /// machine never offers a control that its port would refuse.
    ///
    /// True for both, by different means. A pty publishes its own bytes; a herdr
    /// pane is read on a timer and the difference between two reads is fed to
    /// the same emulator. Neither is visible above this line.
    pub fn can_attach(&self) -> bool {
        true
    }

    /// Whether this backend has a layout engine to split.
    ///
    /// Separate from `can_attach`, which used to stand in for it back when only
    /// one backend could do either. A pty backend has no layout to ask, so it
    /// refuses a split and must not advertise one.
    pub fn can_split(&self) -> bool {
        matches!(self, Self::Herdr(_))
    }

    /// The geometry a pane gets when the backend cannot observe its own.
    pub fn default_size(&self) -> Size {
        match self {
            Self::Herdr(b) => b.default_size(),
            Self::Pty(b) => b.default_size(),
        }
    }

    /// The whole tree in one backend round trip.
    ///
    /// `TerminalBackendTrait` has no snapshot method, and adding one would
    /// oblige every adapter to have a single-call form. This is inherent, so an
    /// adapter that cannot do it in one call is free not to offer it.
    pub fn tree(&self) -> Result<BackendTree, BackendError> {
        match self {
            Self::Herdr(b) => b.tree(),
            Self::Pty(b) => b.tree(),
        }
    }

    /// Whether this backend has a window whose focus a client could move.
    ///
    /// Not implied by `can_split`. A pty's panes are this process's own and
    /// nothing displays them, so there is no focus to move even though the two
    /// answers happen to coincide today.
    pub fn can_focus(&self) -> bool {
        matches!(self, Self::Herdr(_))
    }

    /// One page of a pane's history, oldest first, with the cursor for the
    /// page before it.
    pub fn read(
        &self,
        pane: &PaneId,
        before_line: Option<u32>,
        limit: u16,
    ) -> Result<herdr::ScrollbackPageOf<String>, BackendError> {
        match self {
            Self::Herdr(b) => b.read(pane, before_line, limit),
            // A pty pane's history lives in its emulator, with real styles and a
            // counted length, and the port reads it from there rather than from
            // here. Reaching this is a caller that did not check.
            Self::Pty(_) => Err(BackendError::message(
                "a pty pane's scrollback comes from its emulator, not from the backend",
            )),
        }
    }
}

impl TerminalBackendTrait for TerminalBackend {
    fn list_workspaces(&self) -> anyhow::Result<Vec<Workspace>> {
        match self {
            Self::Herdr(b) => b.list_workspaces(),
            Self::Pty(b) => b.list_workspaces(),
        }
    }

    fn create_workspace(&self, name: &str) -> anyhow::Result<Workspace> {
        match self {
            Self::Herdr(b) => b.create_workspace(name),
            Self::Pty(b) => b.create_workspace(name),
        }
    }

    fn tab_layout(&self, tab_id: &TabId) -> anyhow::Result<TabLayout> {
        match self {
            Self::Herdr(b) => b.tab_layout(tab_id),
            Self::Pty(b) => b.tab_layout(tab_id),
        }
    }

    fn focus_tab(&self, tab_id: &TabId) -> anyhow::Result<()> {
        match self {
            Self::Herdr(b) => b.focus_tab(tab_id),
            Self::Pty(b) => b.focus_tab(tab_id),
        }
    }

    fn list_tabs(&self, workspace_id: &WorkspaceId) -> anyhow::Result<Vec<Tab>> {
        match self {
            Self::Herdr(b) => b.list_tabs(workspace_id),
            Self::Pty(b) => b.list_tabs(workspace_id),
        }
    }

    fn list_panes(&self, tab_id: &TabId) -> anyhow::Result<Vec<Pane>> {
        match self {
            Self::Herdr(b) => b.list_panes(tab_id),
            Self::Pty(b) => b.list_panes(tab_id),
        }
    }

    fn open_pane(
        &self,
        workspace_id: Option<&WorkspaceId>,
        cwd: Option<&str>,
        size: Size,
    ) -> anyhow::Result<Pane> {
        match self {
            Self::Herdr(b) => b.open_pane(workspace_id, cwd, size),
            Self::Pty(b) => b.open_pane(workspace_id, cwd, size),
        }
    }

    fn split(&self, pane_id: &PaneId, direction: SplitDirection) -> anyhow::Result<Pane> {
        match self {
            Self::Herdr(b) => b.split(pane_id, direction),
            Self::Pty(b) => b.split(pane_id, direction),
        }
    }

    fn close(&self, pane_id: &PaneId) -> anyhow::Result<()> {
        match self {
            Self::Herdr(b) => b.close(pane_id),
            Self::Pty(b) => b.close(pane_id),
        }
    }

    fn send_text(&self, pane_id: &PaneId, text: &str) -> anyhow::Result<()> {
        match self {
            Self::Herdr(b) => b.send_text(pane_id, text),
            Self::Pty(b) => b.send_text(pane_id, text),
        }
    }

    fn start_agent(
        &self,
        pane_id: &PaneId,
        spawn: &AgentSpawn,
    ) -> anyhow::Result<Option<ConversationId>> {
        match self {
            Self::Herdr(b) => b.start_agent(pane_id, spawn),
            Self::Pty(b) => b.start_agent(pane_id, spawn),
        }
    }

    fn type_agent_launch(
        &self,
        pane_id: &PaneId,
        spawn: &AgentSpawn,
    ) -> anyhow::Result<Option<ConversationId>> {
        match self {
            Self::Herdr(b) => b.type_agent_launch(pane_id, spawn),
            Self::Pty(b) => b.type_agent_launch(pane_id, spawn),
        }
    }

    fn submit_prompt(&self, pane_id: &PaneId, text: &str) -> anyhow::Result<()> {
        match self {
            Self::Herdr(b) => b.submit_prompt(pane_id, text),
            Self::Pty(b) => b.submit_prompt(pane_id, text),
        }
    }

    fn send_key(&self, pane_id: &PaneId, key: Key, mods: Mods) -> anyhow::Result<()> {
        match self {
            Self::Herdr(b) => b.send_key(pane_id, key, mods),
            Self::Pty(b) => b.send_key(pane_id, key, mods),
        }
    }

    fn screen(&self, pane_id: &PaneId) -> anyhow::Result<String> {
        match self {
            Self::Herdr(b) => b.screen(pane_id),
            Self::Pty(b) => b.screen(pane_id),
        }
    }
}
