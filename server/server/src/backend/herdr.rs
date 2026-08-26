use std::process::Command;
use tethera_common::structs::terminal::{Pane, Tab, Workspace};
use tethera_common::traits::TerminalBackendTrait;

pub struct HerdrBackend {
    binary: String,
}

impl HerdrBackend {
    pub const DEFAULT_BINARY: &'static str = "herdr";

    pub fn new(binary: String) -> Self {
        Self { binary }
    }

    fn run(&self, args: &[&str]) -> anyhow::Result<String> {
        let output = Command::new(&self.binary).args(args).output()?;

        if !output.status.success() {
            anyhow::bail!(
                "herdr {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr)
            );
        }

        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }
}

impl Default for HerdrBackend {
    fn default() -> Self {
        Self::new(Self::DEFAULT_BINARY.to_string())
    }
}

impl TerminalBackendTrait for HerdrBackend {
    fn list_workspaces(&self) -> anyhow::Result<Vec<Workspace>> {
        let _raw = self.run(&["workspace", "list"])?;

        Ok(Vec::new())
    }

    fn create_workspace(&self, name: &str) -> anyhow::Result<Workspace> {
        self.run(&["workspace", "new", name])?;

        Ok(Workspace {
            id: name.to_string(),
            name: name.to_string(),
        })
    }

    fn list_tabs(&self, _workspace_id: &str) -> anyhow::Result<Vec<Tab>> {
        Ok(Vec::new())
    }

    fn list_panes(&self, _tab_id: &str) -> anyhow::Result<Vec<Pane>> {
        Ok(Vec::new())
    }

    fn send_text(&self, pane_id: &str, text: &str) -> anyhow::Result<()> {
        self.run(&["pane", "send-text", pane_id, text])?;

        Ok(())
    }
}
