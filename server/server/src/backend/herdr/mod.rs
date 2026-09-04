pub mod command;
pub mod ids;
pub mod keys;
pub mod mapping;
pub mod scrollback;
pub mod wire;

pub use command::HerdrCommand;
pub use ids::HerdrIds;
pub use keys::HerdrKeys;
pub use mapping::{Foreground, Mapping};
pub use scrollback::{ScrollbackPageOf, ScrollbackWindow};
pub use wire::Snapshot;

use crate::backend::error::BackendError;
use crate::backend::BackendTree;
use tethera_common::structs::terminal::TabLayout;
use tethera_common::protocol::terminal::{Key, Mods};
use tethera_common::structs::agent::AgentSpawn;
use tethera_common::structs::ids::{ConversationId, PaneId, TabId, WorkspaceId};
use tethera_common::traits::AgentTrait;
use tethera_common::structs::terminal::{Pane, Size, SplitDirection, Tab, Workspace};
use tethera_common::traits::TerminalBackendTrait;

/// herdr, driven over its socket API.
///
/// Every list answer is one `api snapshot` and one parse. herdr reports the
/// whole session in that call — workspaces, tabs, panes and layouts — so a tree
/// render costs one subprocess call plus at most one `pane process-info` per
/// tab, rather than a call per rank per row.
pub struct HerdrBackend {
    herdr: HerdrCommand,
    default_size: Size,
}

impl HerdrBackend {
    pub const DEFAULT_BINARY: &'static str = "herdr";
    pub const DEFAULT_SIZE: Size = Size {
        cols: 120,
        rows: 40,
    };

    pub fn new(binary: String, default_size: Size) -> Self {
        Self {
            herdr: HerdrCommand::new(binary),
            default_size,
        }
    }

    /// How long herdr waits for a started agent to be ready for input.
    ///
    /// Measured: a Claude Code start in a directory it already trusts is ready
    /// in a little over three seconds. The margin is for a first run in a new
    /// directory, where the agent stops at its own trust prompt — herdr reports
    /// that as a start that did not become ready, which is the answer a caller
    /// needs rather than a wait that never ends.
    pub const READY_TIMEOUT_MS: u32 = 20_000;

    /// How many times to ask herdr what a pane's shell is before giving up.
    ///
    /// Six at 50ms covers 300ms against a gap measured at 26ms. The cost is
    /// paid only on a refusal, and only until the answer arrives.
    const WRAPPED_ATTEMPTS: u8 = 6;
    const WRAPPED_INTERVAL: std::time::Duration = std::time::Duration::from_millis(50);

    /// How often to ask herdr whether it has attributed a session to a pane a
    /// launch line was typed at.
    ///
    /// Measured: a Claude Code start under the shim was reported with its
    /// session id inside twelve seconds, well within `READY_TIMEOUT_MS`.
    const SESSION_INTERVAL: std::time::Duration = std::time::Duration::from_millis(500);

    pub fn default_size(&self) -> Size {
        self.default_size
    }

    /// Whether the shim owns this pane's shell.
    ///
    /// Asked of herdr rather than read from a registry, because the answer must
    /// not depend on which process is asking. A short-lived CLI holds no pane
    /// and would see every one of them as unwrapped — which is how
    /// `tethera agent spawn` came to fail on a machine where the hook was
    /// installed, taking the one route that cannot work.
    ///
    /// Polled, because a pane that has just been created has a `shell_pid` and
    /// no foreground list yet — and an agent start arrives inside that window.
    /// Measured: empty at 31ms, filled at 57ms.
    ///
    /// False once a name is known and is not the shim, or if herdr cannot
    /// answer at all. A pane this cannot read is a pane whose refusal stands.
    fn wrapped(&self, native_pane: &str) -> bool {
        for attempt in 0..Self::WRAPPED_ATTEMPTS {
            if attempt > 0 {
                std::thread::sleep(Self::WRAPPED_INTERVAL);
            }

            let Ok(body) = self
                .herdr
                .run_json::<wire::ProcessInfoBody>(&["pane", "process-info", "--pane", native_pane])
            else {
                return false;
            };

            if let Some(name) = body.process_info.shell_process_name() {
                return name == crate::terminal::Shim::ARGV0;
            }
        }

        false
    }

    /// Types the launch line at the pane and presses return, in one call.
    ///
    /// `pane run` rather than `send-text` plus an `enter` key: two calls are two
    /// round trips with a shell prompt in between, and a pane that consumed the
    /// text and then missed the return is left with a command line typed and not
    /// run — which reads to a caller exactly like an agent that failed to start.
    ///
    /// The argv reaches herdr as separate arguments, so no shell of ours parses
    /// it. It is built on this machine from `launch_command` and nothing a
    /// client sent reaches it, which is what makes a line safe to type at a
    /// shell at all.
    ///
    /// Then waits for the session, so this answers the same shape a supervised
    /// start does. herdr finds the agent on its own — measured through the
    /// shim, three levels below the pane's shell — but only after the agent has
    /// come up, and returning `None` before then reports a start that worked as
    /// a conversation with no agent.
    fn type_agent_launch(
        &self,
        pane_id: &PaneId,
        spawn: &AgentSpawn,
    ) -> anyhow::Result<Option<ConversationId>> {
        let native = HerdrIds::native_pane(pane_id)?;
        let argv = spawn.agent.launch_command(spawn);
        let args = Self::typed_launch_args(native, &argv)?;

        self.herdr.run(&args.iter().map(String::as_str).collect::<Vec<_>>())?;

        Ok(self.await_session(native))
    }

    /// The conversation herdr eventually attributes to this pane.
    ///
    /// `None` when it never does, which is a real answer and not a failure: an
    /// agent that stops at its own trust prompt is running and has begun no
    /// session. The caller cannot tell that from one that is merely slow, which
    /// is why this bounds the wait rather than reporting a timeout.
    ///
    /// Errors are swallowed on purpose. `agent get` refuses a pane herdr does
    /// not yet consider an agent terminal, and that is the ordinary state for
    /// most of this wait.
    fn await_session(&self, native_pane: &str) -> Option<ConversationId> {
        let deadline =
            std::time::Instant::now() + std::time::Duration::from_millis(Self::READY_TIMEOUT_MS.into());

        while std::time::Instant::now() < deadline {
            if let Ok(body) = self
                .herdr
                .run_json::<wire::AgentBody>(&["agent", "get", native_pane])
            {
                if let Some(conversation) = body
                    .agent
                    .agent_session
                    .as_ref()
                    .and_then(Mapping::conversation_of)
                {
                    return Some(conversation);
                }
            }

            std::thread::sleep(Self::SESSION_INTERVAL);
        }

        None
    }

    /// The herdr call that types a launch line at a pane and runs it.
    ///
    /// `pane run`, never `agent start`. The two are not interchangeable here:
    /// `agent start` inspects the process herdr spawned in the pane and refuses
    /// anything that is not a bare, idle, recognised shell, which is every pane
    /// whose shell is wrapped. This route asks herdr for no judgement at all.
    ///
    /// The argv stays split. Joining it into one string would hand herdr a
    /// sentence to re-split, and an agent flag carrying a space would come
    /// apart at a place nobody chose.
    ///
    /// An associated function taking its inputs, so the choice of subcommand is
    /// checkable without herdr installed.
    pub fn typed_launch_args(
        native_pane: &str,
        argv: &[String],
    ) -> anyhow::Result<Vec<String>> {
        if argv.is_empty() {
            return Err(BackendError::message("that agent names no binary to launch").into());
        }

        let mut args = vec!["pane".to_string(), "run".to_string(), native_pane.to_string()];
        args.extend(argv.iter().cloned());

        Ok(args)
    }

    /// A name herdr will accept for an agent, derived from the pane it runs in.
    ///
    /// herdr requires a lowercase letter first and then only lowercase letters,
    /// digits, `-` and `_`, at most 32 characters. A native pane id is `w6H:p1`,
    /// so lowercasing it and replacing the colon satisfies all of that and stays
    /// unique per pane — a name collision would point a later `agent prompt` at
    /// somebody else's agent.
    fn agent_name(native_pane: &str) -> String {
        native_pane
            .chars()
            .map(|c| match c {
                'A'..='Z' => c.to_ascii_lowercase(),
                'a'..='z' | '0'..='9' | '_' => c,
                _ => '-',
            })
            .take(32)
            .collect()
    }

    /// The whole session, once.
    pub fn snapshot(&self) -> Result<Snapshot, BackendError> {
        let body: wire::SnapshotBody = self.herdr.run_json(&["api", "snapshot"])?;
        let snapshot = body.snapshot;

        if !snapshot.speaks_known_protocol() {
            tracing::warn!(
                herdr_protocol = snapshot.protocol,
                known = ?Snapshot::KNOWN_PROTOCOLS,
                herdr_version = %snapshot.version,
                "herdr speaks a protocol this backend was not written against"
            );
        }

        Ok(snapshot)
    }

    /// What is running in each of the named panes.
    ///
    /// A pane herdr no longer has is simply absent from the map, and its
    /// `foreground_command` is `None`: one dead pane must not blank a listing
    /// that is otherwise true. Anything else — a decode failure above all — is
    /// logged, because an always-empty map that nobody notices is how this
    /// field silently stopped being filled once already.
    pub fn foreground(&self, panes: &[&str]) -> Foreground {
        let mut found = Foreground::new();

        for pane in panes {
            match self
                .herdr
                .run_json::<wire::ProcessInfoBody>(&["pane", "process-info", "--pane", pane])
            {
                Ok(body) => {
                    if let Some(command) = body.process_info.command() {
                        found.insert((*pane).to_string(), command);
                    }
                }
                Err(BackendError::NotFound { .. }) => {}
                Err(error) => tracing::warn!(
                    pane = pane,
                    %error,
                    "herdr could not report what is running in a pane"
                ),
            }
        }

        found
    }

    /// The panes a listing has to ask `process-info` about.
    ///
    /// herdr's own `agent` answers for a pane running one, straight out of the
    /// snapshot, so the calls are only for the panes it does not cover.
    fn needs_process_info<'a>(
        panes: impl Iterator<Item = &'a wire::PaneInfo>,
    ) -> Vec<&'a str> {
        panes
            .filter(|pane| {
                pane.display_agent.as_deref().unwrap_or("").trim().is_empty()
                    && pane.agent.as_deref().unwrap_or("").trim().is_empty()
            })
            .map(|pane| pane.pane_id.as_str())
            .collect()
    }

    /// The primary pane of every tab, which is where a tab row's foreground
    /// command and conversation come from.
    fn tab_primaries(snapshot: &Snapshot) -> Vec<&wire::PaneInfo> {
        snapshot
            .tabs
            .iter()
            .filter_map(|tab| snapshot.primary_pane_of_tab(&tab.tab_id))
            .collect()
    }

    /// One page of a pane's history, oldest first, with the cursor for the page
    /// before it.
    ///
    /// One subprocess call. Nothing is read from `PaneInfo.scroll`: its row
    /// counts are an upper bound rather than a length, and a window planned
    /// from them claims pages that do not exist.
    pub fn read(
        &self,
        pane: &PaneId,
        before_line: Option<u32>,
        limit: u16,
    ) -> Result<ScrollbackPageOf<String>, BackendError> {
        let native = HerdrIds::native_pane(pane)?;
        let window = ScrollbackWindow::plan(before_line, limit);

        if window.limit == 0 {
            return Ok(window.resolve(Vec::new()));
        }

        let requested = window.lines_to_request.to_string();
        let raw = self.herdr.run(&[
            "pane",
            "read",
            native,
            "--source",
            "recent",
            "--lines",
            &requested,
            "--format",
            "text",
        ])?;

        Ok(window.resolve(raw.lines().map(str::to_owned).collect()))
    }

    /// A pane's current content, with the styles it is drawn in.
    ///
    /// Raw ANSI rather than the parsed envelope every other call returns,
    /// because `pane read` answers with the text itself and not with JSON. It is
    /// fed to an emulator, so the escape sequences are the payload.
    ///
    /// `strip_ansi` is not passed: `--format ansi` is what keeps the colours,
    /// and a read without it returns a screen that renders as plain grey.
    /// The whole tree in one snapshot, for the machine port above.
    ///
    /// `TerminalPort` has no `list_workspaces` — `ListWorkspaces` is served
    /// from the machine's tree — so this is the seam that lets one snapshot
    /// answer all three ranks instead of three.
    pub fn tree(&self) -> Result<BackendTree, BackendError> {
        let snapshot = self.snapshot()?;
        let wanted = Self::needs_process_info(Self::tab_primaries(&snapshot).into_iter());
        let foreground = self.foreground(&wanted);

        let tabs = Mapping::tabs(&snapshot, None, &foreground);

        // From this snapshot, not a call per tab. Asking `tab_layout` once per
        // tab would run one `herdr api snapshot` per tab, and a machine watch
        // reads this on a two-second timer for as long as somebody is looking.
        //
        // Driven from the layouts rather than from the tabs, so nothing here can
        // fail: each layout carries its own native tab id and `HerdrIds::tab` is
        // the constructor direction. Going the other way needs `native_tab`,
        // which is fallible, and its only honest failure handling would be to
        // drop a tab's geometry silently.
        let layouts = snapshot
            .layouts
            .iter()
            .map(|layout| Mapping::layout(layout, &HerdrIds::tab(&layout.tab_id)))
            .collect();

        Ok(BackendTree {
            workspaces: Mapping::workspaces(&snapshot),
            tabs,
            panes: Mapping::panes(&snapshot, None, &foreground, self.default_size),
            layouts,
        })
    }

    /// A pane herdr has just made, given its own answer about it.
    ///
    /// One snapshot follows the create, because neither `workspace_created`,
    /// `tab_created` nor a split's `pane_info` carries a rect and the layout is
    /// the only place one exists. This is not a create-then-find: the pane's id
    /// came from the create, and if the layout has not placed it yet the size
    /// falls back to the tab's own area rather than to a fabricated constant.
    fn created(&self, pane: wire::PaneInfo) -> Result<Pane, BackendError> {
        let snapshot = self.snapshot()?;
        let known = snapshot.pane(&pane.pane_id).unwrap_or(&pane);
        let wanted = Self::needs_process_info(std::iter::once(known));

        Ok(Mapping::pane(
            &snapshot,
            known,
            &self.foreground(&wanted),
            self.default_size,
        ))
    }

    fn direction(direction: SplitDirection) -> &'static str {
        match direction {
            SplitDirection::Horizontal => "right",
            SplitDirection::Vertical => "down",
        }
    }
}

impl Default for HerdrBackend {
    fn default() -> Self {
        Self::new(Self::DEFAULT_BINARY.to_string(), Self::DEFAULT_SIZE)
    }
}

impl TerminalBackendTrait for HerdrBackend {
    fn list_workspaces(&self) -> anyhow::Result<Vec<Workspace>> {
        Ok(Mapping::workspaces(&self.snapshot()?))
    }

    fn tab_layout(&self, tab_id: &TabId) -> anyhow::Result<TabLayout> {
        let native = HerdrIds::native_tab(tab_id)?;
        let snapshot = self.snapshot()?;

        let herdr = snapshot
            .layout_of_tab(native)
            .ok_or_else(|| anyhow::anyhow!("herdr placed no panes in tab {native}"))?;

        Ok(Mapping::layout(herdr, tab_id))
    }

    fn focus_tab(&self, tab_id: &TabId) -> anyhow::Result<()> {
        let native = HerdrIds::native_tab(tab_id)?;

        self.herdr.run(&["tab", "focus", native])?;

        Ok(())
    }

    fn create_workspace(&self, name: &str) -> anyhow::Result<Workspace> {
        // A workspace created unfocused never reaches a shell prompt, so the
        // focus happens here, at creation, and is not a request a client can
        // make.
        let created: wire::Created = self.herdr.run_json(&[
            "workspace",
            "create",
            "--label",
            HerdrIds::label(name)?,
            "--focus",
        ])?;

        // The create answers with what it made, and `WorkspaceInfo` plus
        // `root_pane` carry every field of a `Workspace`. Snapshotting to find
        // it would be the create-then-list the protocol forbids.
        let workspace = created
            .workspace
            .ok_or_else(|| BackendError::message("herdr created no workspace"))?;

        Ok(Mapping::workspace(&workspace, Some(&created.root_pane)))
    }

    fn list_tabs(&self, workspace_id: &WorkspaceId) -> anyhow::Result<Vec<Tab>> {
        let native = HerdrIds::native_workspace(workspace_id)?;
        let snapshot = self.snapshot()?;

        let primaries = Self::tab_primaries(&snapshot)
            .into_iter()
            .filter(|pane| pane.workspace_id == native);
        let wanted = Self::needs_process_info(primaries);

        Ok(Mapping::tabs(
            &snapshot,
            Some(native),
            &self.foreground(&wanted),
        ))
    }

    fn list_panes(&self, tab_id: &TabId) -> anyhow::Result<Vec<Pane>> {
        let native = HerdrIds::native_tab(tab_id)?;
        let snapshot = self.snapshot()?;

        let in_tab = snapshot.panes.iter().filter(|pane| pane.tab_id == native);
        let wanted = Self::needs_process_info(in_tab);

        Ok(Mapping::panes(
            &snapshot,
            Some(native),
            &self.foreground(&wanted),
            self.default_size,
        ))
    }

    fn open_pane(
        &self,
        workspace_id: Option<&WorkspaceId>,
        cwd: Option<&str>,
        _size: Size,
    ) -> anyhow::Result<Pane> {
        // The size is not passed on, because herdr accepts none: there is no
        // size argument on any create in the CLI or in the schema. What comes
        // back reports the geometry the pane actually got.
        let workspace = workspace_id.map(HerdrIds::native_workspace).transpose()?;
        let cwd = cwd.map(HerdrIds::cwd).transpose()?;

        let mut args: Vec<&str> = match workspace {
            Some(id) => vec!["tab", "create", "--workspace", id],
            None => vec!["workspace", "create"],
        };

        args.push("--focus");

        if let Some(cwd) = cwd {
            args.push("--cwd");
            args.push(cwd);
        }

        let created: wire::Created = self.herdr.run_json(&args)?;

        Ok(self.created(created.root_pane)?)
    }

    fn split(&self, pane_id: &PaneId, direction: SplitDirection) -> anyhow::Result<Pane> {
        let native = HerdrIds::native_pane(pane_id)?;
        let body: wire::PaneBody = self.herdr.run_json(&[
            "pane",
            "split",
            "--pane",
            native,
            "--direction",
            Self::direction(direction),
            "--focus",
        ])?;

        Ok(self.created(body.pane)?)
    }

    fn close(&self, pane_id: &PaneId) -> anyhow::Result<()> {
        let native = HerdrIds::native_pane(pane_id)?;

        self.herdr.run(&["pane", "close", native])?;

        Ok(())
    }

    fn send_text(&self, pane_id: &PaneId, text: &str) -> anyhow::Result<()> {
        let native = HerdrIds::native_pane(pane_id)?;

        // The text is not guarded. Measured: `pane send-text <id> --version`
        // reaches the pane lookup, so herdr takes this positional with hyphens
        // and all, and a person typing `-v` into a terminal must reach it.
        self.herdr.run(&["pane", "send-text", native, text])?;

        Ok(())
    }

    fn start_agent(
        &self,
        pane_id: &PaneId,
        spawn: &AgentSpawn,
    ) -> anyhow::Result<Option<ConversationId>> {
        let native = HerdrIds::native_pane(pane_id)?;
        let name = Self::agent_name(native);

        let argv = spawn.agent.launch_command(spawn);
        let (kind, flags) = argv
            .split_first()
            .ok_or_else(|| BackendError::message("that agent names no binary to launch"))?;

        let timeout = Self::READY_TIMEOUT_MS.to_string();
        let mut args = vec![
            "agent",
            "start",
            &name,
            "--kind",
            kind,
            "--pane",
            native,
            "--timeout",
            &timeout,
        ];

        // Everything past the separator is the agent's own argv, and nothing a
        // client sent ever reaches it. That is what makes the line safe to type
        // at a shell, so anything added here has to come from this machine.
        if !flags.is_empty() {
            args.push("--");
            args.extend(flags.iter().map(String::as_str));
        }

        let started: Result<wire::AgentBody, _> = self.herdr.run_json(&args);

        let body = match started {
            Ok(body) => body,
            // A refusal the shim explains is routed around. Any other refusal is
            // a pane genuinely running something, and typing a launch line into
            // that would put it wherever the keyboard already is.
            Err(BackendError::NotStartable { message }) => {
                if self.wrapped(native) {
                    return self.type_agent_launch(pane_id, spawn);
                }

                return Err(BackendError::NotStartable { message }.into());
            }
            Err(error) => return Err(error.into()),
        };

        Ok(body
            .agent
            .agent_session
            .as_ref()
            .and_then(Mapping::conversation_of))
    }

    /// herdr's own detection window, which is the region it watches to decide
    /// what an agent is doing.
    ///
    /// Narrower than the whole pane on purpose: it is the part a harness draws
    /// its prompts in, so a detector reading it is not also reading the
    /// conversation scrolling past above.
    fn screen(&self, pane_id: &PaneId) -> anyhow::Result<String> {
        let native = HerdrIds::native_pane(pane_id)?;

        Ok(self.herdr.run(&[
            "agent",
            "read",
            native,
            "--source",
            "detection",
            "--format",
            "text",
        ])?)
    }

    fn send_key(&self, pane_id: &PaneId, key: Key, mods: Mods) -> anyhow::Result<()> {
        let native = HerdrIds::native_pane(pane_id)?;
        let named = HerdrKeys::name(key, mods)?;

        self.herdr.run(&["pane", "send-keys", native, &named])?;

        Ok(())
    }

    fn submit_prompt(&self, pane_id: &PaneId, text: &str) -> anyhow::Result<()> {
        let native = HerdrIds::native_pane(pane_id)?;

        // The prompt is a positional argument to a process this backend spawns
        // with `Command::args`, so no shell parses it: a newline stays a
        // newline and a backtick stays a backtick. Typing it at the pane
        // instead would submit each line as its own prompt, and would put a
        // string a phone sent in front of a shell.
        self.herdr.run(&["agent", "prompt", native, text])?;

        Ok(())
    }
}
