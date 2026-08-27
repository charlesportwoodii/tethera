use super::ForegroundProcess;
use serde::Deserialize;

/// What is running in a pane.
///
/// The only source of a foreground command in herdr's API, and there is no bulk
/// form: `PaneProcessInfoParams` takes one optional `pane_id`, so this costs one
/// call per pane.
#[derive(Debug, Clone, Deserialize)]
pub struct ProcessInfo {
    pub pane_id: String,
    #[serde(default)]
    pub shell_pid: Option<u32>,
    #[serde(default)]
    pub tty: Option<String>,
    #[serde(default)]
    pub foreground_process_group_id: Option<u32>,
    #[serde(default)]
    pub foreground_processes: Vec<ForegroundProcess>,
}

impl ProcessInfo {
    /// The program a person would say is running here.
    ///
    /// `name` rather than `cmdline`: a tab row draws `cargo`, not
    /// `"C:\Users\charl\.cargo\bin\cargo.exe" test --workspace`. The
    /// executable suffix is dropped so the same program reads the same on every
    /// platform.
    pub fn command(&self) -> Option<String> {
        let name = self.foreground_processes.first()?.name.as_str();
        let trimmed = name
            .strip_suffix(".exe")
            .or_else(|| name.strip_suffix(".EXE"))
            .unwrap_or(name);

        (!trimmed.is_empty()).then(|| trimmed.to_owned())
    }
}
