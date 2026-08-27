use serde::Deserialize;

/// One process in a pane's foreground group.
#[derive(Debug, Clone, Deserialize)]
pub struct ForegroundProcess {
    pub pid: u32,
    pub name: String,
    #[serde(default)]
    pub argv0: Option<String>,
    #[serde(default)]
    pub argv: Option<Vec<String>>,
    #[serde(default)]
    pub cmdline: Option<String>,
    #[serde(default)]
    pub cwd: Option<String>,
}
