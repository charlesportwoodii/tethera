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
    /// Whether the process herdr spawned in this pane is one of this name.
    ///
    /// The question behind an `agent_pane_busy` refusal. herdr will not start an
    /// agent in a pane whose spawned process it does not recognise as a bare
    /// idle shell, and it never recognises the shim — so a refusal on a pane the
    /// shim owns is structural, not a shell that happens to be busy.
    ///
    /// **It says nothing about whether the pane is busy, and cannot.** Measured
    /// under a `default_shell` hook, with `ping -n 30` running inside the shim
    /// and printing to the pane: herdr still reported one foreground process,
    /// the shim itself, at `shell_pid`. Its tree walk starts at the process it
    /// spawned and does not descend through a name it does not know, so the
    /// inner shell and everything under it are invisible here.
    ///
    /// The consequence is the caller's to accept: typing a launch line at a
    /// wrapped pane may type it into whatever already holds the keyboard. That
    /// is a person's own pane, on screen in front of them, and they asked — the
    /// alternative is that an agent can never be started in a relayed pane at
    /// all. A busy check would need the shim to report it, since the shim is the
    /// only thing on that side that can see.
    pub fn shell_is_process(&self, name: &str) -> bool {
        self.shell_process_name() == Some(name.to_ascii_lowercase())
    }

    /// The name of the process herdr spawned in this pane, when it says.
    ///
    /// `None` means herdr has not reported one — **not** that the pane has no
    /// shell. Measured on a pane 31ms old: `shell_pid` was already set and
    /// `foreground_processes` was empty, and the list filled 26ms later. A
    /// caller that reads that absence as "no shim here" sends a freshly created
    /// pane down the supervised route that cannot work, which is exactly how
    /// `tethera agent spawn` failed intermittently.
    pub fn shell_process_name(&self) -> Option<String> {
        let shell = self.shell_pid?;

        self.foreground_processes
            .iter()
            .find(|process| process.pid == shell)
            .map(|process| Self::stem(&process.name))
    }

    /// A process name with its path and executable suffix removed, lowercased.
    fn stem(name: &str) -> String {
        let base = name.rsplit(['/', '\\']).next().unwrap_or(name);

        base.strip_suffix(".exe")
            .or_else(|| base.strip_suffix(".EXE"))
            .unwrap_or(base)
            .to_ascii_lowercase()
    }

    pub fn command(&self) -> Option<String> {
        let name = self.foreground_processes.first()?.name.as_str();
        let trimmed = name
            .strip_suffix(".exe")
            .or_else(|| name.strip_suffix(".EXE"))
            .unwrap_or(name);

        (!trimmed.is_empty()).then(|| trimmed.to_owned())
    }
}
