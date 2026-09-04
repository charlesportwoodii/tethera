use crate::protocol::terminal::{Key, Mods};
use crate::structs::agent::AgentSpawn;
use crate::structs::ids::{ConversationId, PaneId, TabId, WorkspaceId};
use crate::structs::terminal::{Pane, Size, SplitDirection, Tab, TabLayout, Workspace};

/// What a terminal multiplexer has to be able to do.
///
/// Two constraints an implementation must encode, which the protocol
/// deliberately does not:
///
/// 1. The link to the backend is one serialised connection and a blocking call
///    holds it. Long calls are the implementation's problem to make
///    non-blocking or to report as busy; they are never allowed to become a
///    silent stall on the wire.
/// 2. A workspace created unfocused does not reach a shell prompt, so
///    `open_pane` focuses what it creates. Focus is not a client operation.
pub trait TerminalBackendTrait {
    fn list_workspaces(&self) -> anyhow::Result<Vec<Workspace>>;

    fn create_workspace(&self, name: &str) -> anyhow::Result<Workspace>;

    fn list_tabs(&self, workspace_id: &WorkspaceId) -> anyhow::Result<Vec<Tab>>;

    fn list_panes(&self, tab_id: &TabId) -> anyhow::Result<Vec<Pane>>;

    /// Where this tab's panes sit, in cells.
    ///
    /// An error rather than an empty layout when the backend does not track
    /// geometry: a client told "no panes" draws an empty workspace, which is a
    /// different and wrong statement.
    fn tab_layout(&self, tab_id: &TabId) -> anyhow::Result<TabLayout>;

    /// Move the backend's own focus to this tab.
    fn focus_tab(&self, tab_id: &TabId) -> anyhow::Result<()>;

    /// Creates a new tab. A second pane inside an existing tab is `split`, which
    /// is the only operation that needs a direction.
    fn open_pane(
        &self,
        workspace_id: Option<&WorkspaceId>,
        cwd: Option<&str>,
        size: Size,
    ) -> anyhow::Result<Pane>;

    fn split(&self, pane_id: &PaneId, direction: SplitDirection) -> anyhow::Result<Pane>;

    fn close(&self, pane_id: &PaneId) -> anyhow::Result<()>;

    fn send_text(&self, pane_id: &PaneId, text: &str) -> anyhow::Result<()>;

    /// Starts an agent at a pane's interactive shell prompt, and returns once it
    /// is ready for input.
    ///
    /// Separate from `send_text` because typing the command is not the whole of
    /// it. A backend that can tell a started agent from a shell that swallowed
    /// the line must not be reduced to typing and hoping, and the caller has no
    /// other way to know the difference.
    ///
    /// `spawn.prompt` is deliberately not part of this. A launch line is typed
    /// at a shell, so a prompt that arrived from a phone would be typed there
    /// too, where `;` and a backtick are commands. The prompt is delivered with
    /// `submit_prompt` once the agent owns the keyboard.
    ///
    /// Answers the conversation the started agent announced, when it announced
    /// one in time. `None` is a real answer and not a failure: an agent that
    /// stops at its own trust prompt is running and has begun no session, and
    /// nothing here can tell that apart from one that is merely slow.
    fn start_agent(
        &self,
        pane_id: &PaneId,
        spawn: &AgentSpawn,
    ) -> anyhow::Result<Option<ConversationId>>;

    /// Starts an agent by typing its launch line at the pane's shell.
    ///
    /// Typing and hoping, which is what `start_agent` exists to avoid — so this
    /// is for the panes where a supervised start is not on offer at all, not a
    /// simpler alternative to it.
    ///
    /// A multiplexer decides whether a pane is startable by inspecting the
    /// process it spawned there, and a pane whose shell is wrapped fails that
    /// inspection however healthy the shell inside is. Measured against herdr:
    /// `available_pane_shell` requires the spawned process to carry one of
    /// fifteen known shell names and, on Windows, to have no descendants at
    /// all. A wrapper has one by definition. The refusal arrives as
    /// `agent_pane_busy`, and no amount of retrying changes it.
    ///
    /// Returns `None` for the same reason a byte-stream backend does: nothing
    /// here can tell a started agent from a shell that printed "command not
    /// found". The multiplexer discovers the agent afterwards by its own means
    /// — measured: herdr labels a pane `agent: claude` from process inspection
    /// three levels below the shell it spawned — so the session identity
    /// arrives on a later poll rather than in this answer.
    fn type_agent_launch(
        &self,
        pane_id: &PaneId,
        spawn: &AgentSpawn,
    ) -> anyhow::Result<Option<ConversationId>>;

    /// Hands text to the agent running in a pane and submits it.
    ///
    /// Not `send_text` plus a carriage return: an agent's editor treats a
    /// newline inside the text as a submission, so a two-line prompt sent that
    /// way arrives as two prompts, the first of them truncated.
    fn submit_prompt(&self, pane_id: &PaneId, text: &str) -> anyhow::Result<()>;

    /// Sends one key press to a pane.
    ///
    /// Not expressible through `send_text`, and deliberately so: text has every
    /// control character stripped out of it before it reaches a pane, because a
    /// client that could put an escape byte in a string could drive the program
    /// on the other end. Escape, Enter and the arrows therefore have no route
    /// through text at all — and those are exactly the keys an interrupt and an
    /// answer to a question are made of.
    ///
    /// A backend that cannot express a key says so. Refusing is the honest
    /// answer, because a key silently dropped reaches the caller as a keystroke
    /// that was delivered and did nothing.
    fn send_key(&self, pane_id: &PaneId, key: Key, mods: Mods) -> anyhow::Result<()>;

    /// What the agent in a pane currently has on screen, as text.
    ///
    /// Some of what an agent puts in front of a person is never written down: a
    /// permission prompt lives on the screen and nowhere else, and the review
    /// step of a question set is drawn rather than recorded. Reading is the only
    /// way to know either is there — and reading beats predicting, because a
    /// rule inferred from today's harness would act on whatever is on screen the
    /// day that changes.
    fn screen(&self, pane_id: &PaneId) -> anyhow::Result<String>;
}
