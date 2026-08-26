mod herdr;

pub use herdr::HerdrBackend;

use tethera_common::structs::terminal::{Pane, Tab, Workspace};
use tethera_common::traits::TerminalBackendTrait;

pub enum TerminalBackend {
    Herdr(HerdrBackend),
}

impl TerminalBackendTrait for TerminalBackend {
    fn list_workspaces(&self) -> anyhow::Result<Vec<Workspace>> {
        match self {
            Self::Herdr(b) => b.list_workspaces(),
        }
    }

    fn create_workspace(&self, name: &str) -> anyhow::Result<Workspace> {
        match self {
            Self::Herdr(b) => b.create_workspace(name),
        }
    }

    fn list_tabs(&self, workspace_id: &str) -> anyhow::Result<Vec<Tab>> {
        match self {
            Self::Herdr(b) => b.list_tabs(workspace_id),
        }
    }

    fn list_panes(&self, tab_id: &str) -> anyhow::Result<Vec<Pane>> {
        match self {
            Self::Herdr(b) => b.list_panes(tab_id),
        }
    }

    fn send_text(&self, pane_id: &str, text: &str) -> anyhow::Result<()> {
        match self {
            Self::Herdr(b) => b.send_text(pane_id, text),
        }
    }
}
