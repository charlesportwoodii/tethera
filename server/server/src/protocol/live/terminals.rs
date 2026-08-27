use std::sync::{Arc, Mutex};
use std::time::Duration;

use tethera_common::protocol::capability::{self, CapabilityId, CapabilitySet};
use tethera_common::protocol::error::{EntityKind, WireError};
use tethera_common::protocol::terminal::{
    attrs, AttachSpec, Color, Key, Mods, RowUpdate, Span, Style,
};
use tethera_common::protocol::view::PaneView;
use tethera_common::structs::agent::AgentSpawn;
use tethera_common::structs::ids::{ConversationId, PaneId, TabId, WorkspaceId};
use tethera_common::structs::terminal::{Pane, SplitDirection, Tab, TabLayout};
use tethera_common::traits::TerminalBackendTrait;
use tokio::sync::Semaphore;

use crate::backend::{BackendError, BackendTree, TerminalBackend};
use crate::config::{ApplicationConfig, TerminalKind};
use crate::protocol::live::{HerdrSession, LiveSession};
use crate::protocol::ports::{ScrollbackPage, TerminalPort};
use crate::terminal::{HerdrSource, PaneRegistry, PtyBackend};

/// `TerminalPort` over a real terminal backend.
///
/// Everything this port does is a synchronous subprocess call one layer down,
/// so every call runs the blocking work off the runtime's threads and gives up
/// on a deadline. A slow backend call is never allowed to become a silent stall
/// on the wire.
pub struct LiveTerminals {
    backend: Arc<TerminalBackend>,
    /// The emulators for panes this machine owns.
    ///
    /// Shared with the backend, which adopts into it when it opens a pane, so
    /// emulation starts at a pane's first byte rather than at its first attach.
    panes: Arc<PaneRegistry>,
    gate: Arc<Semaphore>,
    deadline: Duration,
    /// The panes of the last tree this port read.
    ///
    /// Kept so the conversation port can answer which pane a conversation is
    /// bound to without a second backend call. A tree read is a subprocess round
    /// trip under an admission gate, and `MachinePort::tree` already makes one
    /// immediately before asking for the conversation rank; a second would
    /// double every render of the home screen.
    seen: Mutex<Vec<Pane>>,
}

impl LiveTerminals {
    /// A spawn cap, not a serialised link.
    ///
    /// The predecessor held one persistent socket to herdr, and spec § 12's
    /// "one serialised connection" describes that. This backend runs an
    /// independent `herdr` process per call, so there is nothing to serialise —
    /// what needs bounding is how many processes a burst of requests can start
    /// at once. A phone drawing a tree while a scrollback page is in flight
    /// legitimately reaches several.
    pub const DEFAULT_PERMITS: usize = 8;

    /// Longer than the backend's own per-call deadline, so a call herdr was
    /// killed for reports why rather than being cut off here first.
    pub const DEFAULT_DEADLINE: Duration = Duration::from_secs(10);

    /// How long a start may take before it is reported as contention.
    ///
    /// Wider than every other call because it is not a snapshot: the backend
    /// waits for the started agent to be ready for input, and that wait is what
    /// the caller asked for. Wider than the backend's own readiness timeout too,
    /// so a start that gave up reports why it gave up rather than being cut off
    /// here first and reported as contention.
    pub const START_DEADLINE: Duration = Duration::from_secs(30);

    /// The page size this machine advertises in `Describe.limits`.
    ///
    /// Clamped rather than trusted: a client asking for 65535 lines would
    /// otherwise produce a request for 65535 lines.
    pub const MAX_SCROLLBACK_PAGE: u16 = 500;

    /// The one style every scrollback row indexes.
    ///
    /// herdr answers `pane read` as text. There is no way in this protocol to
    /// say "styling unknown", so a page says what it can say truthfully: the
    /// text, against the renderer's own colours.
    const PLAIN: Style = Style {
        fg: Color::Default,
        bg: Color::Default,
        attrs: attrs::NONE,
    };

    pub fn new(backend: Arc<TerminalBackend>, panes: Arc<PaneRegistry>) -> Self {
        Self::bounded(backend, panes, Self::DEFAULT_PERMITS, Self::DEFAULT_DEADLINE)
    }

    pub fn new_shared(backend: Arc<TerminalBackend>, panes: Arc<PaneRegistry>) -> Arc<Self> {
        Arc::new(Self::new(backend, panes))
    }

    /// The terminal stack this machine's configuration asks for.
    ///
    /// Assembled here rather than at each call site, so a binary that drives
    /// panes without serving a connection — the CLI — reaches the same backend
    /// the phone does, and a new backend kind is added in one place. The two
    /// halves are built together because the pty backend adopts into the
    /// registry as it opens a pane and cannot be constructed without it.
    pub fn from_config(config: &ApplicationConfig) -> Arc<Self> {
        let panes = PaneRegistry::new_shared();
        let backend = Arc::new(match config.terminal_backend {
            TerminalKind::Herdr => {
                TerminalBackend::herdr(config.herdr_binary.clone(), config.terminal_size)
            }
            TerminalKind::Pty => TerminalBackend::pty(
                panes.clone(),
                config.terminal_size,
                PtyBackend::default_shell(),
            ),
        });

        Self::new_shared(backend, panes)
    }

    /// The same port with the admission gate and the deadline named.
    ///
    /// A behavioural seam rather than a builder: `Busy` under contention is a
    /// documented answer, and there is no other way to observe it without
    /// wedging a real herdr.
    pub fn bounded(
        backend: Arc<TerminalBackend>,
        panes: Arc<PaneRegistry>,
        permits: usize,
        deadline: Duration,
    ) -> Self {
        Self {
            backend,
            panes,
            gate: Arc::new(Semaphore::new(permits.max(1))),
            deadline,
            seen: Mutex::new(Vec::new()),
        }
    }

    /// Which conversation each pane is running, as of the last tree read.
    ///
    /// A read of memory, never a backend call. `None` for a pane whose agent has
    /// not announced its session - `herdr integration install <agent>` is what
    /// makes it announce, and until it is installed a live pane is honestly
    /// unbound rather than guessed at from its working directory.
    pub fn bindings(&self) -> Vec<(PaneId, ConversationId)> {
        self.seen
            .lock()
            .expect("lock")
            .iter()
            .filter_map(|pane| {
                pane.conversation
                    .as_ref()
                    .map(|conversation| (pane.id.clone(), conversation.clone()))
            })
            .collect()
    }

    /// The panes of the last tree read, whole.
    ///
    /// `bindings` answers the identified ones; this answers all of them, which
    /// is what a caller needs when the interesting panes are precisely the ones
    /// carrying an agent that named no session.
    pub fn panes(&self) -> Vec<Pane> {
        self.seen.lock().expect("lock").clone()
    }

    /// What this port genuinely does, for the machine's `Describe`.
    ///
    /// Per backend, because the two differ in exactly the places a client draws a
    /// control. Advertising a capability whose port refuses is the one thing a
    /// capability set exists to prevent, so `terminal_attach` and
    /// `terminal_input` appear only where a pane really has a byte stream, and
    /// `pane_split` disappears where there is no layout to split.
    pub fn capabilities(&self) -> CapabilitySet {
        let mut named = vec![
            capability::PANE_OPEN,
            capability::PANE_CLOSE,
            capability::TERMINAL_SCROLLBACK,
        ];

        if self.backend.can_attach() {
            named.push(capability::TERMINAL_ATTACH);
            named.push(capability::TERMINAL_INPUT);
        }

        // Asked of the backend rather than inferred from attach. The two were
        // the same question while only one backend could attach; they are not
        // the same question now, and reading one off the other would quietly
        // stop advertising a split that works.
        if self.backend.can_split() {
            named.push(capability::PANE_SPLIT);
            // The same fact, not a second one: a backend with a layout engine is
            // exactly a backend that has a layout to report.
            named.push(capability::PANE_LAYOUT);
        }

        // A different fact, and it gets its own question. A pty's panes are this
        // process's own and nothing displays them, so there is no focus to move
        // even though the two answers happen to coincide today.
        if self.backend.can_focus() {
            named.push(capability::TAB_FOCUS);
        }

        if self.backend.can_lines_view() {
            named.push(capability::TERMINAL_LINES_VIEW);
        }

        named.into_iter().map(CapabilityId::from).collect()
    }

    /// Every rank of the tree from one backend round trip, for the machine port
    /// above, which is what `Request::ListWorkspaces` is served from.
    pub async fn tree(&self) -> Result<BackendTree, WireError> {
        let tree = self
            .run(|backend| backend.tree().map_err(anyhow::Error::from))
            .await?;

        *self.seen.lock().expect("lock") = tree.panes.clone();

        Ok(tree)
    }

    /// Starts an agent in a pane, and answers the conversation it announced.
    ///
    /// Its own deadline, because this one is not a snapshot call: herdr waits
    /// for the agent to be ready for input, and that wait is the point. Under
    /// the ordinary deadline every start would be reported as `Busy` while the
    /// agent was still coming up.
    pub async fn start_agent(
        &self,
        pane: &PaneId,
        spawn: &AgentSpawn,
    ) -> Result<Option<ConversationId>, WireError> {
        let pane = pane.clone();
        let spawn = spawn.clone();

        self.run_within(Self::START_DEADLINE, move |backend| {
            backend.start_agent(&pane, &spawn)
        })
        .await
    }

    /// Sends one key press to a pane.
    pub async fn send_key(&self, pane: &PaneId, key: Key, mods: Mods) -> Result<(), WireError> {
        let pane = pane.clone();

        self.run(move |backend| backend.send_key(&pane, key, mods))
            .await
    }

    /// What a pane currently has on screen, as text.
    pub async fn screen(&self, pane: &PaneId) -> Result<String, WireError> {
        let pane = pane.clone();

        self.run(move |backend| backend.screen(&pane)).await
    }

    /// Types text into a pane without submitting it.
    ///
    /// Not `submit_prompt`: this goes into whatever field the pane has focused,
    /// which is how the free-text row of a question is filled in.
    pub async fn send_text(&self, pane: &PaneId, text: &str) -> Result<(), WireError> {
        let pane = pane.clone();
        let text = text.to_owned();

        self.run(move |backend| backend.send_text(&pane, &text))
            .await
    }

    /// Hands a prompt to the agent in a pane and submits it.
    pub async fn submit_prompt(&self, pane: &PaneId, text: &str) -> Result<(), WireError> {
        let pane = pane.clone();
        let text = text.to_owned();

        self.run(move |backend| backend.submit_prompt(&pane, &text))
            .await
    }

    /// Runs one backend call off the runtime, under the gate, against the
    /// deadline.
    ///
    /// The gate waits rather than refusing outright — reaching the cap is
    /// normal under load, and an immediate refusal there would not be honest —
    /// but it waits against the same deadline as the call, so contention that
    /// does not clear surfaces as `Busy` instead of a queue nobody can see. The
    /// backend kills its own child on a shorter deadline, so a permit is
    /// released rather than held for the life of the process.
    async fn run<T, F>(&self, work: F) -> Result<T, WireError>
    where
        T: Send + 'static,
        F: FnOnce(&TerminalBackend) -> anyhow::Result<T> + Send + 'static,
    {
        self.run_within(self.deadline, work).await
    }

    /// `run` against a deadline the caller names, for the calls whose duration
    /// is the work rather than a symptom of contention.
    async fn run_within<T, F>(&self, deadline: Duration, work: F) -> Result<T, WireError>
    where
        T: Send + 'static,
        F: FnOnce(&TerminalBackend) -> anyhow::Result<T> + Send + 'static,
    {
        let permit = tokio::time::timeout(deadline, Arc::clone(&self.gate).acquire_owned())
            .await
            .map_err(|_| WireError::Busy)?
            .map_err(|_| WireError::Backend {
                message: "the terminal backend is shutting down".to_string(),
            })?;

        let backend = Arc::clone(&self.backend);

        let handle = tokio::task::spawn_blocking(move || {
            let outcome = work(&backend);
            drop(permit);

            outcome
        });

        match tokio::time::timeout(deadline, handle).await {
            Ok(Ok(Ok(value))) => Ok(value),
            Ok(Ok(Err(error))) => Err(BackendError::classify(error)),
            Ok(Err(error)) => Err(WireError::Backend {
                message: format!("terminal backend task failed: {error}"),
            }),
            Err(_) => Err(WireError::Busy),
        }
    }

    /// Text as rows the client can draw, one default style for all of them.
    fn rows(lines: Vec<String>) -> (Vec<Style>, Vec<RowUpdate>) {
        let rows = lines
            .into_iter()
            .enumerate()
            .map(|(index, text)| RowUpdate {
                y: u16::try_from(index).unwrap_or(u16::MAX),
                from_x: 0,
                spans: vec![Span { style: 0, text }],
            })
            .collect();

        (vec![Self::PLAIN], rows)
    }
}

impl TerminalPort for LiveTerminals {
    type Session = LiveSession;

    async fn list_tabs(&self, workspace: &WorkspaceId) -> Result<Vec<Tab>, WireError> {
        let workspace = workspace.clone();

        self.run(move |backend| backend.list_tabs(&workspace)).await
    }

    async fn list_panes(&self, tab: &TabId) -> Result<Vec<Pane>, WireError> {
        let tab = tab.clone();

        self.run(move |backend| backend.list_panes(&tab)).await
    }

    async fn layout(&self, tab: &TabId) -> Result<TabLayout, WireError> {
        let tab = tab.clone();

        self.run(move |backend| backend.tab_layout(&tab)).await
    }

    async fn focus_tab(&self, tab: &TabId) -> Result<(), WireError> {
        let tab = tab.clone();

        self.run(move |backend| backend.focus_tab(&tab)).await
    }

    async fn open(
        &self,
        workspace: Option<&WorkspaceId>,
        cwd: Option<&str>,
    ) -> Result<Pane, WireError> {
        let workspace = workspace.cloned();
        let cwd = cwd.map(str::to_owned);

        self.run(move |backend| {
            // The size is the answer for a pane the backend cannot observe.
            // herdr accepts no requested geometry on a create, so what comes
            // back reports what the pane actually is.
            let size = backend.default_size();

            backend.open_pane(workspace.as_ref(), cwd.as_deref(), size)
        })
        .await
    }

    async fn split(&self, pane: &PaneId, direction: SplitDirection) -> Result<Pane, WireError> {
        let pane = pane.clone();

        self.run(move |backend| backend.split(&pane, direction))
            .await
    }

    async fn close(&self, pane: &PaneId) -> Result<(), WireError> {
        let pane = pane.clone();

        self.run(move |backend| backend.close(&pane)).await
    }

    async fn attach(&self, spec: &AttachSpec) -> Result<Self::Session, WireError> {
        // Keyed on which backend owns the feed, not on whether the registry
        // already holds this pane. `holds` is true for a pty pane from the
        // moment it is opened and true for a herdr pane once anybody has looked
        // at it - so gating on it skipped `ensure` for every *re*-attach, and a
        // re-attach carrying a different view is exactly what the view toggle
        // is. That is what made the toggle inert even after `ensure` itself
        // learned to notice a changed shape.
        if self.backend.is_pulled() {
            if !self.backend.can_attach() {
                return Err(WireError::NotFound {
                    kind: EntityKind::Pane,
                });
            }

            // `Lines` needs a backend that can return output with its wrapping
            // removed. Refusing here rather than quietly serving the other view
            // keeps the answer honest: `terminal_lines_view` says which machines
            // offer it, and a client that ignored that gets told.
            if spec.view == PaneView::Lines && !self.backend.can_lines_view() {
                return Err(WireError::Backend {
                    message: "this machine cannot return output with its wrapping removed"
                        .to_string(),
                });
            }

            HerdrSource::ensure(
                Arc::clone(&self.backend),
                Arc::clone(&self.panes),
                Arc::clone(&self.gate),
                spec.pane.clone(),
                spec.view,
                spec.viewport,
            );
        } else if !self.panes.holds(&spec.pane) {
            // A pushed pane is adopted when it is opened, so one the registry
            // does not hold is one this machine does not have.
            return Err(WireError::NotFound {
                kind: EntityKind::Pane,
            });
        }

        let frames = self.panes.attach(&spec.pane)?;

        // Input splits here and nowhere else. A pty takes the bytes the emulator
        // encoded; herdr takes key names, and the emulator is a reader of its
        // panes rather than their owner, so anything written into it would reach
        // nobody.
        if self.backend.can_lines_view() {
            return Ok(LiveSession::Herdr(HerdrSession::new(
                frames,
                Arc::clone(&self.backend),
                Arc::clone(&self.gate),
                spec.pane.clone(),
            )));
        }

        Ok(LiveSession::Direct(frames))
    }

    async fn scrollback(
        &self,
        pane: &PaneId,
        before_line: Option<u32>,
        limit: u16,
    ) -> Result<ScrollbackPage, WireError> {
        let limit = limit.min(Self::MAX_SCROLLBACK_PAGE);

        // A pane this machine emulates has real styles and a counted length. A
        // pane it only observes has neither, and falling through to the backend
        // is better than degrading both answers to the weaker one.
        if self.panes.holds(pane) {
            return self.panes.scrollback(pane, before_line, limit);
        }

        let pane = pane.clone();

        let page = self
            .run(move |backend| {
                backend
                    .read(&pane, before_line, limit)
                    .map_err(anyhow::Error::from)
            })
            .await?;

        let (styles, rows) = Self::rows(page.lines);

        Ok((styles, rows, page.next_before_line, page.has_earlier))
    }
}
