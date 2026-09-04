use crate::state::AppState;
use tauri::{AppHandle, State};
use tethera_client_core::rpc::Rpc;
use tethera_common::protocol::capability::{self, HasCapability};
use tethera_common::protocol::response::Payload;
use tethera_common::protocol::terminal::{AttachSpec, Key, Mods, TerminalInput};
use tethera_common::protocol::Request;
use tethera_common::structs::client::{MachineTree, TerminalControls};
use tethera_common::structs::ids::{PaneId, ServerId, TabId, WorkspaceId};
use tethera_common::structs::terminal::{
    Pane, Size, SplitDirection, Tab, TabLayout, Workspace,
};

/// Parsing at the boundary rather than passing strings inwards.
///
/// The prefix is part of the value, so a tab id handed in where a pane id
/// belongs fails here by name instead of reaching the machine and resolving to
/// nothing.
struct Ids;

impl Ids {
    fn server(value: &str) -> Result<ServerId, String> {
        ServerId::parse(value).ok_or_else(|| format!("{value} is not a server id"))
    }

    fn pane(value: &str) -> Result<PaneId, String> {
        PaneId::parse(value).ok_or_else(|| format!("{value} is not a pane id"))
    }

    fn tab(value: &str) -> Result<TabId, String> {
        TabId::parse(value).ok_or_else(|| format!("{value} is not a tab id"))
    }

    fn workspace(value: &str) -> Result<WorkspaceId, String> {
        WorkspaceId::parse(value).ok_or_else(|| format!("{value} is not a workspace id"))
    }
}

#[tauri::command]
pub(crate) async fn list_workspaces(
    state: State<'_, AppState>,
    server: String,
) -> Result<Vec<Workspace>, String> {
    let server = Ids::server(&server)?;
    let connection = state.connect(&server).await?;

    match Rpc::request(&connection, Request::ListWorkspaces)
        .await
        .map_err(|error| error.to_string())?
    {
        Payload::Workspaces(workspaces) => Ok(workspaces),
        other => Err(format!("the machine answered with {other:?}")),
    }
}

#[tauri::command]
pub(crate) async fn list_tabs(
    state: State<'_, AppState>,
    server: String,
    workspace: String,
) -> Result<Vec<Tab>, String> {
    let server = Ids::server(&server)?;
    let connection = state.connect(&server).await?;

    let request = Request::ListTabs {
        workspace: Ids::workspace(&workspace)?,
    };

    match Rpc::request(&connection, request)
        .await
        .map_err(|error| error.to_string())?
    {
        Payload::Tabs(tabs) => Ok(tabs),
        other => Err(format!("the machine answered with {other:?}")),
    }
}

#[tauri::command]
pub(crate) async fn list_panes(
    state: State<'_, AppState>,
    server: String,
    tab: String,
) -> Result<Vec<Pane>, String> {
    let server = Ids::server(&server)?;
    let connection = state.connect(&server).await?;

    let request = Request::ListPanes {
        tab: Ids::tab(&tab)?,
    };

    match Rpc::request(&connection, request)
        .await
        .map_err(|error| error.to_string())?
    {
        Payload::Panes(panes) => Ok(panes),
        other => Err(format!("the machine answered with {other:?}")),
    }
}

/// Where a tab's panes sit, in cells.
#[tauri::command]
pub(crate) async fn pane_layout(
    state: State<'_, AppState>,
    server: String,
    tab: String,
) -> Result<TabLayout, String> {
    let server = Ids::server(&server)?;
    let connection = state.connect(&server).await?;

    let request = Request::PaneLayout {
        tab: Ids::tab(&tab)?,
    };

    match Rpc::request(&connection, request)
        .await
        .map_err(|error| error.to_string())?
    {
        Payload::Layout(layout) => Ok(layout),
        other => Err(format!("the machine answered with {other:?}")),
    }
}

/// Move the machine's own focus to this tab.
#[tauri::command]
pub(crate) async fn focus_tab(
    state: State<'_, AppState>,
    server: String,
    tab: String,
) -> Result<(), String> {
    let server = Ids::server(&server)?;
    let connection = state.connect(&server).await?;

    let request = Request::FocusTab {
        tab: Ids::tab(&tab)?,
    };

    match Rpc::request(&connection, request)
        .await
        .map_err(|error| error.to_string())?
    {
        Payload::Ack => Ok(()),
        other => Err(format!("the machine answered with {other:?}")),
    }
}

/// Subscribes to a machine's whole tree, and answers with its opening snapshot.
///
/// The tree is not push-based on the machine either — it diffs successive reads
/// — so what this buys is that the reads happen while somebody is watching
/// rather than only when a screen asks. Without it a tab closed at the desk
/// stays on the phone until something unrelated triggers a fetch.
#[tauri::command]
pub(crate) async fn watch_machine(
    app: AppHandle,
    state: State<'_, AppState>,
    server: String,
) -> Result<MachineTree, String> {
    let id = Ids::server(&server)?;
    let connection = state.connect(&id).await?;

    state
        .machine_watch()
        .start(app, connection, server)
        .await
}

#[tauri::command]
pub(crate) async fn unwatch_machine(state: State<'_, AppState>) -> Result<(), String> {
    state.machine_watch().stop().await;

    Ok(())
}

/// What this machine will let a terminal screen do.
#[tauri::command]
pub(crate) async fn terminal_controls(
    state: State<'_, AppState>,
    server: String,
) -> Result<TerminalControls, String> {
    let server = Ids::server(&server)?;
    let connection = state.connect(&server).await?;

    let Payload::Describe(describe) = Rpc::request(&connection, Request::Describe)
        .await
        .map_err(|error| error.to_string())?
    else {
        return Err("the machine did not describe itself".to_string());
    };

    let has = |name: &str| describe.capabilities.has(name);

    Ok(TerminalControls {
        attach: has(capability::TERMINAL_ATTACH),
        input: has(capability::TERMINAL_INPUT),
        scrollback: has(capability::TERMINAL_SCROLLBACK),
        open: has(capability::PANE_OPEN),
        split: has(capability::PANE_SPLIT),
        close: has(capability::PANE_CLOSE),
        layout: has(capability::PANE_LAYOUT),
        focus_tab: has(capability::TAB_FOCUS),
    })
}

/// Opens a live stream of one pane's screen.
///
/// Frames arrive on `PaneAttachments::CHANNEL` rather than as a return value:
/// there is no last frame, and a command that returned one would have to be
/// called forever.
#[tauri::command]
pub(crate) async fn attach_pane(
    app: AppHandle,
    state: State<'_, AppState>,
    server: String,
    pane: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    let server = Ids::server(&server)?;
    let connection = state.connect(&server).await?;

    let spec = AttachSpec {
        pane: Ids::pane(&pane)?,
        viewport: Size { cols, rows },
    };

    state.panes().start(app, connection, spec).await
}

/// Stops reading a pane. The pane keeps running on the machine.
#[tauri::command]
pub(crate) async fn detach_pane(state: State<'_, AppState>, pane: String) -> Result<(), String> {
    state.panes().stop(&Ids::pane(&pane)?).await;

    Ok(())
}

#[tauri::command]
pub(crate) async fn pane_key(
    state: State<'_, AppState>,
    pane: String,
    key: Key,
    mods: Mods,
) -> Result<(), String> {
    state
        .panes()
        .send(&Ids::pane(&pane)?, TerminalInput::Key { key, mods })
        .await
}

#[tauri::command]
pub(crate) async fn pane_text(
    state: State<'_, AppState>,
    pane: String,
    text: String,
) -> Result<(), String> {
    state
        .panes()
        .send(&Ids::pane(&pane)?, TerminalInput::Text(text))
        .await
}

/// A new tab, in the named workspace or in a new one.
#[tauri::command]
pub(crate) async fn open_terminal(
    state: State<'_, AppState>,
    server: String,
    workspace: Option<String>,
    cwd: Option<String>,
) -> Result<Pane, String> {
    let server = Ids::server(&server)?;
    let connection = state.connect(&server).await?;

    let workspace = match workspace {
        Some(value) => Some(Ids::workspace(&value)?),
        None => None,
    };

    match Rpc::request(&connection, Request::OpenTerminal { workspace, cwd })
        .await
        .map_err(|error| error.to_string())?
    {
        Payload::Pane(pane) => Ok(pane),
        other => Err(format!("the machine answered with {other:?}")),
    }
}

/// A second pane inside an existing tab.
///
/// The handoff case in both directions: a split made here is a real pane on the
/// machine, and appears at the desk.
#[tauri::command]
pub(crate) async fn split_pane(
    state: State<'_, AppState>,
    server: String,
    pane: String,
    direction: SplitDirection,
) -> Result<Pane, String> {
    let server = Ids::server(&server)?;
    let connection = state.connect(&server).await?;

    let request = Request::SplitPane {
        pane: Ids::pane(&pane)?,
        direction,
    };

    match Rpc::request(&connection, request)
        .await
        .map_err(|error| error.to_string())?
    {
        Payload::Pane(pane) => Ok(pane),
        other => Err(format!("the machine answered with {other:?}")),
    }
}

#[tauri::command]
pub(crate) async fn close_pane(
    state: State<'_, AppState>,
    server: String,
    pane: String,
) -> Result<(), String> {
    let server = Ids::server(&server)?;
    let connection = state.connect(&server).await?;

    let pane = Ids::pane(&pane)?;

    // Stopped first, so the pump is not reading a stream the machine is about to
    // end. It would survive that - the stream ends and the pump breaks - but the
    // screen would be told the attach failed rather than that the pane closed.
    state.panes().stop(&pane).await;

    match Rpc::request(&connection, Request::ClosePane { pane })
        .await
        .map_err(|error| error.to_string())?
    {
        Payload::Ack => Ok(()),
        other => Err(format!("the machine answered with {other:?}")),
    }
}
