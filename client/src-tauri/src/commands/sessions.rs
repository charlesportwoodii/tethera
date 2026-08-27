use crate::state::AppState;
use tauri::State;
use tethera_client_core::error::ClientError;
use tethera_client_core::rpc::Rpc;
use tethera_common::protocol::capability::{self, HasCapability};
use tethera_common::protocol::response::{ConversationPreview, Payload};
use tethera_common::protocol::{Request, WireError};
use tethera_common::structs::agent::AgentProfile;
use tethera_common::structs::client::StartOutcome;
use tethera_common::structs::ids::{ProfileId, ServerId};

/// What this machine will actually run.
///
/// Asked rather than assumed: a machine without Codex installed simply does not
/// list it, and the screen offers only what will work.
#[tauri::command]
pub(crate) async fn list_agent_profiles(
    state: State<'_, AppState>,
    id: String,
) -> Result<Vec<AgentProfile>, String> {
    let id = ServerId::parse(&id).ok_or_else(|| format!("{id} is not a server id"))?;
    let connection = state.connect(&id).await?;

    let payload = Rpc::request(&connection, Request::ListAgentProfiles)
        .await
        .map_err(|error| error.to_string())?;

    match payload {
        Payload::AgentProfiles(profiles) => Ok(profiles),
        other => Err(format!("the machine answered with {other:?}")),
    }
}

/// Whether a machine can start a session at all.
///
/// Read from the capabilities recorded at the last handshake rather than by
/// dialling: the screen needs this before it draws, and a machine whose
/// transcript reader is not running cannot start anything.
#[tauri::command]
pub(crate) async fn can_start_sessions(
    state: State<'_, AppState>,
    id: String,
) -> Result<bool, String> {
    let id = ServerId::parse(&id).ok_or_else(|| format!("{id} is not a server id"))?;

    Ok(state
        .entry(&id)?
        .capabilities
        .has(capability::CONVERSATION_START))
}

/// Directories this machine has been worked in, newest first.
///
/// Asked rather than derived from the conversations already on screen. The
/// machine drops a directory that no longer exists, and `StartConversation`
/// refuses one — offering a choice the next call rejects is worse than offering
/// nothing.
#[tauri::command]
pub(crate) async fn recent_cwds(
    state: State<'_, AppState>,
    id: String,
    limit: u16,
) -> Result<Vec<String>, String> {
    let id = ServerId::parse(&id).ok_or_else(|| format!("{id} is not a server id"))?;

    if !state.entry(&id)?.capabilities.has(capability::RECENT_CWDS) {
        return Ok(Vec::new());
    }

    let connection = state.connect(&id).await?;

    let payload = Rpc::request(&connection, Request::RecentCwds { limit })
        .await
        .map_err(|error| error.to_string())?;

    match payload {
        Payload::RecentCwds(paths) => Ok(paths),
        other => Err(format!("the machine answered with {other:?}")),
    }
}

/// What starting this would create, without creating it.
///
/// The workspace and tab names come from the machine because the machine
/// generates them. A client that guessed them would be right until the day it
/// was not, and the person would only find out after the pane existed.
#[tauri::command]
pub(crate) async fn preview_conversation(
    state: State<'_, AppState>,
    id: String,
    profile: String,
    cwd: String,
) -> Result<Option<ConversationPreview>, String> {
    let id = ServerId::parse(&id).ok_or_else(|| format!("{id} is not a server id"))?;

    if !state
        .entry(&id)?
        .capabilities
        .has(capability::CONVERSATION_PREVIEW)
    {
        return Ok(None);
    }

    let connection = state.connect(&id).await?;

    let payload = Rpc::request(
        &connection,
        Request::PreviewConversation {
            profile: ProfileId(profile),
            cwd,
            workspace: None,
        },
    )
    .await
    .map_err(|error| error.to_string())?;

    match payload {
        Payload::ConversationPreview(preview) => Ok(Some(preview)),
        other => Err(format!("the machine answered with {other:?}")),
    }
}

/// Starts an agent on a machine.
///
/// The one command on this screen that changes anything. Everything before it is
/// selection, which is why the screen says nothing starts until it is pressed.
#[tauri::command]
pub(crate) async fn start_conversation(
    state: State<'_, AppState>,
    id: String,
    profile: String,
    cwd: String,
    prompt: Option<String>,
) -> Result<StartOutcome, String> {
    let id = ServerId::parse(&id).ok_or_else(|| format!("{id} is not a server id"))?;
    let connection = state.connect(&id).await?;

    let answer = Rpc::request(
        &connection,
        Request::StartConversation {
            profile: ProfileId(profile),
            cwd,
            // An empty box is no first message, not an empty one. Sending "" as
            // a prompt would make the agent answer nothing.
            prompt: prompt.filter(|text| !text.trim().is_empty()),
            attachments: Vec::new(),
        },
    )
    .await;

    match answer {
        Ok(Payload::Conversation(conversation)) => Ok(StartOutcome::Started(conversation)),
        Ok(other) => Err(format!("the machine answered with {other:?}")),
        // Not a failure: the pane is open and the harness is in it. Kept as a
        // variant rather than a message, so the screen branches on the shape
        // instead of on the machine's prose.
        Err(ClientError::Wire(WireError::AwaitingAgent { pane })) => {
            Ok(StartOutcome::AwaitingAgent { pane })
        }
        Err(error) => Err(error.to_string()),
    }
}
