use super::ids::HerdrIds;
use super::wire::{AgentSession, AgentSessionKind, PaneInfo, Snapshot, WorkspaceInfo};
use std::collections::BTreeMap;
use tethera_common::structs::agent::Agent;
use tethera_common::structs::ids::{ConversationId, ProfileId};
use tethera_common::structs::terminal::{Pane, Size, Tab, Workspace};
use tethera_common::traits::agent::AgentTrait;

/// herdr pane id to the command running in it, for the panes that needed a
/// `pane process-info` call to answer.
///
/// Carried in rather than looked up, because `api snapshot` reports no
/// foreground command and only `pane process-info` answers it, one pane at a
/// time. Keeping it a parameter is what leaves every function here pure.
pub type Foreground = BTreeMap<String, String>;

/// herdr's session as tethera's normalised model.
///
/// Every function is pure over a parsed `Snapshot`, which is what makes the
/// whole mapping testable against committed real output with no herdr on the
/// box.
pub struct Mapping;

impl Mapping {
    pub fn workspaces(snapshot: &Snapshot) -> Vec<Workspace> {
        snapshot
            .workspaces
            .iter()
            .map(|info| {
                Self::workspace(info, snapshot.primary_pane_of_tab(&info.active_tab_id))
            })
            .collect()
    }

    /// One workspace.
    ///
    /// Takes the primary pane rather than reading it, so a create can build its
    /// answer from the create envelope instead of snapshotting to find what it
    /// just made.
    pub fn workspace(info: &WorkspaceInfo, primary: Option<&PaneInfo>) -> Workspace {
        Workspace {
            id: HerdrIds::workspace(&info.workspace_id),
            name: info.label.clone(),
            // herdr records no working directory on a workspace. Its worktree
            // checkout is the only one it has; the active tab's pane is where a
            // person would say the workspace is.
            cwd: info
                .worktree
                .as_ref()
                .map(|tree| tree.checkout_path.clone())
                .or_else(|| primary.and_then(|pane| Self::text(pane.cwd.as_deref()))),
            tab_count: Self::count(info.tab_count),
            conversation: primary.and_then(Self::conversation),
        }
    }

    pub fn tabs(
        snapshot: &Snapshot,
        workspace: Option<&str>,
        foreground: &Foreground,
    ) -> Vec<Tab> {
        snapshot
            .tabs
            .iter()
            .filter(|info| workspace.map_or(true, |id| info.workspace_id == id))
            .map(|info| {
                let primary = snapshot.primary_pane_of_tab(&info.tab_id);

                Tab {
                    id: HerdrIds::tab(&info.tab_id),
                    workspace_id: HerdrIds::workspace(&info.workspace_id),
                    // herdr's own ordinal, not this record's position.
                    // Measured: closing tab 2 of four left the survivors
                    // numbered 1, 3, 4, so the number is stable and a
                    // positional index would have renumbered them.
                    index: Self::count(info.number),
                    title: info.label.clone(),
                    conversation: primary.and_then(Self::conversation),
                    foreground_command: primary
                        .and_then(|pane| Self::command(pane, foreground)),
                }
            })
            .collect()
    }

    pub fn panes(
        snapshot: &Snapshot,
        tab: Option<&str>,
        foreground: &Foreground,
        fallback: Size,
    ) -> Vec<Pane> {
        snapshot
            .panes
            .iter()
            .filter(|info| tab.map_or(true, |id| info.tab_id == id))
            .map(|info| Self::pane(snapshot, info, foreground, fallback))
            .collect()
    }

    pub fn pane(
        snapshot: &Snapshot,
        info: &PaneInfo,
        foreground: &Foreground,
        fallback: Size,
    ) -> Pane {
        Pane {
            id: HerdrIds::pane(&info.pane_id),
            tab_id: HerdrIds::tab(&info.tab_id),
            workspace_id: HerdrIds::workspace(&info.workspace_id),
            // `label` is a required field with no honest absence to report, so
            // an unlabelled pane is named by its id rather than by a blank
            // string pretending to be a name.
            label: Self::text(info.label.as_deref())
                .unwrap_or_else(|| info.pane_id.clone()),
            title: Self::title(info),
            cwd: Self::text(info.cwd.as_deref()),
            size: Self::size(snapshot, info, fallback),
            focused: info.focused,
            foreground_command: Self::command(info, foreground),
            conversation: Self::conversation(info),
            agent: Self::agent(info),
        }
    }

    /// Which agent is running here, whether or not it announced a session.
    ///
    /// Read from herdr's own agent field and mapped through the catalog, so a
    /// name this build does not know is no agent rather than a profile nothing
    /// can describe. Deliberately independent of `conversation`: herdr commonly
    /// reports a running agent it has no session identity for, and collapsing
    /// the two would make a live agent nobody can name look like an empty
    /// shell.
    fn agent(info: &PaneInfo) -> Option<ProfileId> {
        let named = Self::text(info.agent.as_deref())?;

        Agent::ALL
            .into_iter()
            .map(|agent| agent.profile().id)
            .find(|profile| profile.as_str().eq_ignore_ascii_case(&named))
    }

    /// What is running in this pane.
    ///
    /// herdr's own `agent` first: it costs nothing, it is already in the
    /// snapshot, and `claude` is what a tab row should read. `process-info`'s
    /// process name is the answer for a pane with no agent, and it is why the
    /// map exists.
    fn command(info: &PaneInfo, foreground: &Foreground) -> Option<String> {
        Self::text(info.display_agent.as_deref())
            .or_else(|| Self::text(info.agent.as_deref()))
            .or_else(|| foreground.get(&info.pane_id).cloned())
    }

    /// herdr's observed geometry, because that is what the pane actually is.
    ///
    /// herdr accepts no requested size on any create, and splitting a pane
    /// re-lays-out its neighbours, so geometry is an observation that moves
    /// rather than a property fixed at creation. The tab's whole area answers
    /// for a pane the layout has not placed yet — a pane created a moment ago —
    /// because a single-pane tab fills its area exactly, and `viewport_rows`
    /// matched the rect height in every pane observed.
    fn size(snapshot: &Snapshot, info: &PaneInfo, fallback: Size) -> Size {
        let layout = snapshot.layout_of_tab(&info.tab_id);

        if let Some(rect) = layout.and_then(|layout| layout.rect_of(&info.pane_id)) {
            return Size {
                cols: rect.width,
                rows: rect.height,
            };
        }

        let rows = info
            .scroll
            .and_then(|scroll| u16::try_from(scroll.viewport_rows).ok())
            .filter(|rows| *rows > 0);

        match (layout, rows) {
            (Some(layout), rows) => Size {
                cols: layout.area.width,
                rows: rows.unwrap_or(layout.area.height),
            },
            (None, Some(rows)) => Size {
                cols: fallback.cols,
                rows,
            },
            (None, None) => fallback,
        }
    }

    /// The pane's own title, then the terminal's with its status glyph removed,
    /// then the terminal's raw. A pane with none of them has no title, and says
    /// so rather than rendering a blank label.
    fn title(info: &PaneInfo) -> Option<String> {
        Self::text(info.title.as_deref())
            .or_else(|| Self::text(info.terminal_title_stripped.as_deref()))
            .or_else(|| Self::text(info.terminal_title.as_deref()))
    }

    /// The agent's own session identity, when the agent announced one.
    ///
    /// herdr does not discover this; it is populated only by
    /// `herdr pane report-agent-session`, and no live Claude Code pane on the
    /// development box reported it. So this is `None` in practice today, and
    /// the rule below is the contract rather than the observation.
    fn conversation(info: &PaneInfo) -> Option<ConversationId> {
        info.agent_session.as_ref().and_then(Self::conversation_of)
    }

    /// The minting rule the transcript reader has to match, or the tree's
    /// `conversation` points at nothing.
    ///
    /// A path is reduced to its file stem, because that stem is the session id
    /// for every agent that records one — Claude Code writes
    /// `~/.claude/projects/<project>/<session>.jsonl`.
    pub fn conversation_of(session: &AgentSession) -> Option<ConversationId> {
        let value = match session.kind {
            AgentSessionKind::Id => Self::text(Some(session.value.as_str()))?,
            AgentSessionKind::Path => {
                let stem = std::path::Path::new(&session.value)
                    .file_stem()
                    .map(|stem| stem.to_string_lossy().into_owned())?;

                Self::text(Some(stem.as_str()))?
            }
            AgentSessionKind::Unknown => return None,
        };

        Some(ConversationId::mint(&value))
    }

    /// An empty string is an absence herdr spelled badly, never a value.
    fn text(value: Option<&str>) -> Option<String> {
        value
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
    }

    /// Saturating rather than wrapping: a count beyond `u16` is wrong, and
    /// `u16::MAX` is at least wrong in the direction a reader can see.
    fn count(value: u64) -> u16 {
        u16::try_from(value).unwrap_or(u16::MAX)
    }
}
