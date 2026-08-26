use crate::structs::terminal::{Pane, Tab, Workspace};

pub trait TerminalBackendTrait {
    fn list_workspaces(&self) -> anyhow::Result<Vec<Workspace>>;

    fn create_workspace(&self, name: &str) -> anyhow::Result<Workspace>;

    fn list_tabs(&self, workspace_id: &str) -> anyhow::Result<Vec<Tab>>;

    fn list_panes(&self, tab_id: &str) -> anyhow::Result<Vec<Pane>>;

    fn send_text(&self, pane_id: &str, text: &str) -> anyhow::Result<()>;
}
