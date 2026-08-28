mod pane;

pub use pane::PtyPane;

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use tethera_common::protocol::terminal::{Key, Mods};
use tethera_common::structs::agent::AgentSpawn;
use tethera_common::structs::ids::{ConversationId, PaneId, TabId, WorkspaceId};
use tethera_common::structs::terminal::{
    Pane, PaneRect, PaneSlot, Size, SplitDirection, Tab, TabLayout, Workspace,
};
use tethera_common::traits::{AgentTrait, TerminalBackendTrait};

use crate::backend::{BackendError, BackendTree};
use crate::terminal::keys::KeyEncoder;
use crate::terminal::registry::PaneRegistry;
use crate::terminal::source::PaneSource;

/// A terminal backend over ptys this process owns.
///
/// The only backend that can be attached to. herdr publishes no per-pane byte
/// stream, so a machine that needs a live terminal drives this instead.
///
/// The tree is flat on purpose: one workspace, one tab per pane, one pane per tab.
/// There is no layout engine here, so `split` refuses rather than inventing a
/// geometry, and `pane_split` is not advertised for this backend.
pub struct PtyBackend {
    registry: Arc<PaneRegistry>,
    /// A `Mutex` and not a bare map for two reasons: every trait method takes
    /// `&self` while `open_pane` and `close` mutate, and `MasterPty` is `Send` but
    /// not `Sync`, so without the mutex `TerminalBackend` would stop being `Sync`
    /// and every `spawn_blocking` call site above would fail to compile.
    panes: Mutex<HashMap<PaneId, Entry>>,
    default_size: Size,
    shell: String,
    workspace: WorkspaceId,
    /// `PaneId::mint` is a plain prefix plus its argument, so a fixed suffix
    /// would hand every pane the same id and each new pane would replace the
    /// last in the map. The counter is what makes an id an identity.
    next: AtomicU64,
}

struct Entry {
    pty: PtyPane,
    tab: TabId,
    label: String,
    cwd: Option<String>,
    /// The ordinal a person reads, fixed when the pane was opened.
    ///
    /// `Tab.index` is "the backend's own ordinal", and an index taken from list
    /// position renumbers when a tab closes — turning somebody's `2:build` into
    /// `1:build`. Over a `HashMap` it is worse than that: iteration order is
    /// arbitrary and shifts on rehash, so the renumbering is not even monotonic,
    /// and because `Tab` compares by value every tab would then diff as changed
    /// whenever any pane opened or closed.
    serial: u16,
}

impl PtyBackend {
    /// How many panes may be open at once.
    ///
    /// A memory and handle bound rather than a policy: each pane is a pty, a
    /// child process, three OS threads and an emulator holding 2000 lines of
    /// scrollback. An operator driving agents from a phone will not reach this; a
    /// runaway loop will.
    ///
    /// It bounds *concurrent* panes only. Panes that have closed are reaped, so
    /// this is not what stops a long session accumulating them - `reap` is.
    pub const MAX_PANES: usize = 64;

    pub const DEFAULT_SIZE: Size = Size {
        cols: 120,
        rows: 40,
    };

    pub fn new(registry: Arc<PaneRegistry>, default_size: Size, shell: String) -> Self {
        Self {
            registry,
            panes: Mutex::new(HashMap::new()),
            default_size,
            shell,
            workspace: WorkspaceId::mint("local"),
            next: AtomicU64::new(1),
        }
    }

    /// The interactive shell this machine would start.
    ///
    /// A missing variable must not mean a missing pane, so each platform has a
    /// fallback that is present on every install of it.
    pub fn default_shell() -> String {
        if cfg!(windows) {
            std::env::var("ComSpec").unwrap_or_else(|_| "cmd.exe".to_string())
        } else {
            std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string())
        }
    }

    pub fn default_size(&self) -> Size {
        self.default_size
    }

    fn panes(&self) -> MutexGuard<'_, HashMap<PaneId, Entry>> {
        self.panes
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Drops entries for panes that are no longer being emulated.
    ///
    /// A pane whose shell exits on its own is removed from the registry by its
    /// own pump, and nothing tells this map. Without a reap the entry lives for
    /// the life of the process, and it does three bad things: it holds the pty
    /// master open, which is what keeps the reader thread blocked in `read`
    /// forever; it counts against `MAX_PANES`; and `tree` reports a pane that no
    /// longer exists.
    ///
    /// The registry is asked rather than told, so the two maps converge without a
    /// callback in either direction. Dropping the entry drops the master, and a
    /// dropped master is what lets the reader return - measured: a reader whose
    /// child was killed stays blocked while the master is alive and exits once it
    /// is dropped.
    fn reap(&self) {
        let mut held = self.panes();
        let dead: Vec<PaneId> = held
            .keys()
            .filter(|pane| !self.registry.holds(pane))
            .cloned()
            .collect();

        for pane in dead {
            held.remove(&pane);
        }
    }

    fn workspace(&self) -> Workspace {
        let mut workspace = Workspace::new(self.workspace.clone(), "local".to_string());
        workspace.tab_count = u16::try_from(self.panes().len()).unwrap_or(u16::MAX);

        workspace
    }

    fn describe(&self, id: &PaneId, entry: &Entry) -> Pane {
        Pane {
            id: id.clone(),
            tab_id: entry.tab.clone(),
            workspace_id: self.workspace.clone(),
            label: entry.label.clone(),
            title: None,
            cwd: entry.cwd.clone(),
            // Chosen by this backend and stable for the pane's life, because this
            // backend owns the pty. That is the one place spec 10.1 holds.
            size: entry.pty.size(),
            focused: false,
            foreground_command: None,
            conversation: None,
            agent: None,
        }
    }

    /// Every rank of this backend's tree, for the port above.
    pub fn tree(&self) -> Result<BackendTree, BackendError> {
        self.reap();

        let held = self.panes();
        let mut tabs = Vec::with_capacity(held.len());
        let mut panes = Vec::with_capacity(held.len());

        for (id, entry) in held.iter() {
            tabs.push(Tab {
                id: entry.tab.clone(),
                workspace_id: self.workspace.clone(),
                index: entry.serial,
                title: entry.label.clone(),
                conversation: None,
                foreground_command: None,
            });
            panes.push(self.describe(id, entry));
        }

        // Ordered by the ordinal rather than left in `HashMap` order, so two
        // reads of an unchanged tree are equal and the watcher does not report
        // every tab as changed.
        tabs.sort_by_key(|tab| tab.index);
        panes.sort_by(|left, right| left.tab_id.as_str().cmp(right.tab_id.as_str()));

        drop(held);

        Ok(BackendTree {
            workspaces: vec![self.workspace()],
            tabs,
            panes,
            // This backend has no layout engine, so it places nothing. An empty
            // list is the true answer about it rather than a read that failed.
            layouts: Vec::new(),
        })
    }
}

impl TerminalBackendTrait for PtyBackend {
    fn list_workspaces(&self) -> anyhow::Result<Vec<Workspace>> {
        Ok(vec![self.workspace()])
    }

    /// A pty tab has exactly the geometry of its one pane, and none at all past
    /// that.
    ///
    /// This backend owns no split tree. Reporting a made-up arrangement for two
    /// panes would put a map on screen that somebody would trust, so the second
    /// pane is an error instead. `pane_layout` is not advertised for a machine
    /// running this backend, so a client never asks in the first place.
    fn tab_layout(&self, tab_id: &TabId) -> anyhow::Result<TabLayout> {
        let panes = self.list_panes(tab_id)?;

        let [pane] = panes.as_slice() else {
            anyhow::bail!(
                "this backend tracks no geometry for a tab of {} panes",
                panes.len()
            );
        };

        Ok(TabLayout {
            tab: tab_id.clone(),
            slots: vec![PaneSlot {
                pane: pane.id.clone(),
                rect: PaneRect::new(0, 0, pane.size.cols, pane.size.rows),
            }],
            zoomed: None,
        })
    }

    /// Nothing here has focus to move: these panes are this process's own and
    /// no window shows them.
    fn focus_tab(&self, _tab_id: &TabId) -> anyhow::Result<()> {
        anyhow::bail!("this backend has no focus to move")
    }

    /// There is one workspace and it always exists.
    ///
    /// Refusing would be worse than answering: a caller asking for a workspace on
    /// a backend with exactly one wants somewhere to put a pane, and it has one.
    fn create_workspace(&self, _name: &str) -> anyhow::Result<Workspace> {
        Ok(self.workspace())
    }

    fn list_tabs(&self, workspace_id: &WorkspaceId) -> anyhow::Result<Vec<Tab>> {
        if *workspace_id != self.workspace {
            return Err(BackendError::NotFound {
                kind: tethera_common::protocol::error::EntityKind::Workspace,
            }
            .into());
        }

        Ok(self.tree()?.tabs)
    }

    fn list_panes(&self, tab_id: &TabId) -> anyhow::Result<Vec<Pane>> {
        self.reap();

        let held = self.panes();
        let panes: Vec<Pane> = held
            .iter()
            .filter(|(_, entry)| entry.tab == *tab_id)
            .map(|(id, entry)| self.describe(id, entry))
            .collect();

        Ok(panes)
    }

    fn open_pane(
        &self,
        _workspace_id: Option<&WorkspaceId>,
        cwd: Option<&str>,
        size: Size,
    ) -> anyhow::Result<Pane> {
        // Before the cap is checked, or a machine that has opened and closed 64
        // panes over a long session refuses to open a 65th while holding none.
        self.reap();

        if self.panes().len() >= Self::MAX_PANES {
            tracing::warn!(
                cap = Self::MAX_PANES,
                "refusing to open a pane: every closed pane keeps an OS thread on Windows"
            );

            return Err(BackendError::message(format!(
                "this machine will not hold more than {} open panes",
                Self::MAX_PANES
            ))
            .into());
        }

        // Clamped once, here, and the clamped value is what the pty, the
        // emulator and `Pane.size` all get. `PtySize` accepts a zero dimension and
        // `Buffer` clamps to 1, so an unclamped 0x0 would report a geometry no
        // part of the stack actually has.
        let size = Size {
            cols: size.cols.max(1),
            rows: size.rows.max(1),
        };

        // Relaxed is enough: nothing's correctness depends on observing a
        // particular ordering of these, only on each value being handed out once.
        let serial = self.next.fetch_add(1, Ordering::Relaxed);
        let id = PaneId::mint(&format!("pty{serial}"));
        let (pty, io) = PtyPane::open(&self.shell, cwd, size)?;

        // Adopted before this returns, so emulation starts at the pane's first
        // byte. Waiting for an attach would leave the shell's own banner and
        // prompt with nowhere to go.
        self.registry.adopt(id.clone(), io, PaneSource::Streamed);

        let entry = Entry {
            pty,
            tab: TabId::mint(&format!("pty{serial}")),
            label: self.shell.clone(),
            cwd: cwd.map(str::to_owned),
            serial: u16::try_from(serial).unwrap_or(u16::MAX),
        };
        let pane = self.describe(&id, &entry);
        self.panes().insert(id, entry);

        Ok(pane)
    }

    // There is no layout engine here to ask for a geometry, and inventing one
    // would report a pane that does not look the way it says it does.
    fn split(&self, _pane_id: &PaneId, _direction: SplitDirection) -> anyhow::Result<Pane> {
        Err(BackendError::message("a pty backend has no layout to split").into())
    }

    fn close(&self, pane_id: &PaneId) -> anyhow::Result<()> {
        let mut held = self.panes();
        let Some(mut entry) = held.remove(pane_id) else {
            return Err(BackendError::NotFound {
                kind: tethera_common::protocol::error::EntityKind::Pane,
            }
            .into());
        };

        drop(held);

        // Released here rather than left to the pump. The pump removes its own
        // entry when the waiter reports an exit, but that needs the child to
        // really exit and `kill` cannot report whether it did — so on a kill that
        // failed the emulator, its pump and its scrollback would live for the
        // rest of the process while nothing counted them. The pump's later
        // removal is a no-op because it checks identity.
        self.registry.forget(pane_id);

        let killed = entry.pty.kill();

        // Explicit, and ordered after the kill, because dropping the master is
        // what lets this pane's reader thread return from `read`. Leaving it to
        // the end of scope would work today and would break the moment somebody
        // returns early above.
        drop(entry);

        killed
    }

    fn send_text(&self, pane_id: &PaneId, text: &str) -> anyhow::Result<()> {
        let held = self.panes();
        let Some(entry) = held.get(pane_id) else {
            return Err(BackendError::NotFound {
                kind: tethera_common::protocol::error::EntityKind::Pane,
            }
            .into());
        };

        // The same sanitising a `TerminalInput::Text` gets, because this reaches
        // the same pty and a control character here would be the same injection.
        //
        // Never bracketed, even on a pane that asked for bracketed paste. This is
        // a caller typing on a pane's behalf - "run this command" - not a person
        // pasting, and bracketing it is precisely what would stop the command
        // being executed.
        entry.pty.write(KeyEncoder::text(text, false))
    }

    /// Types the launch line and presses return.
    ///
    /// No readiness to report and no session to announce: this backend watches
    /// a byte stream, not an agent lifecycle, so it cannot tell a started agent
    /// from a shell that printed "command not found". `None` says so, and the
    /// caller decides what an unannounced start means.
    fn start_agent(
        &self,
        pane_id: &PaneId,
        spawn: &AgentSpawn,
    ) -> anyhow::Result<Option<ConversationId>> {
        let argv = spawn.agent.launch_command(spawn);

        self.send_text(pane_id, &format!("{}\r", argv.join(" ")))?;

        Ok(None)
    }

    /// Types the prompt and presses return.
    ///
    /// Bracketed, unlike `send_text`, and for the opposite reason: this text is
    /// a person's message rather than a command, so an agent's editor has to
    /// receive it as one paste. Unbracketed, every newline in it would submit
    /// the lines before it as prompts of their own.
    /// Rendered from the emulator this backend is already running.
    ///
    /// The one backend that can answer this without asking anybody: it owns the
    /// pty, so the grid is in memory and current.
    fn screen(&self, pane_id: &PaneId) -> anyhow::Result<String> {
        self.registry.screen_of(pane_id).ok_or_else(|| {
            BackendError::NotFound {
                kind: tethera_common::protocol::error::EntityKind::Pane,
            }
            .into()
        })
    }

    /// Encoded for the program on the far end rather than named.
    ///
    /// This backend owns the pty, so it knows the two modes that change what a
    /// key means — DECCKM for the arrows and bracketed paste for text — and
    /// reads both off the emulator that is already parsing the stream. A table
    /// lookup would rediscover a value this server itself set.
    fn send_key(&self, pane_id: &PaneId, key: Key, mods: Mods) -> anyhow::Result<()> {
        let held = self.panes();
        let Some(entry) = held.get(pane_id) else {
            return Err(BackendError::NotFound {
                kind: tethera_common::protocol::error::EntityKind::Pane,
            }
            .into());
        };

        let (application_cursor_keys, _) = self.registry.modes_of(pane_id).unwrap_or((false, false));

        entry.pty.write(KeyEncoder::key(key, mods, application_cursor_keys))
    }

    fn submit_prompt(&self, pane_id: &PaneId, text: &str) -> anyhow::Result<()> {
        let held = self.panes();
        let Some(entry) = held.get(pane_id) else {
            return Err(BackendError::NotFound {
                kind: tethera_common::protocol::error::EntityKind::Pane,
            }
            .into());
        };

        entry.pty.write(KeyEncoder::text(text, true))?;
        entry.pty.write(KeyEncoder::text("\r", false))
    }
}
