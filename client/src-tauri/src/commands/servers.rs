use crate::state::AppState;
use tauri::State;
use tethera_client_core::rpc::Rpc;
use tethera_client_core::sweep::{Sweep, SweepBudget};
use tethera_common::protocol::response::{Page, Payload};
use tethera_common::protocol::Request;
use tethera_common::structs::conversation::{Conversation, ConversationFilter};
use tethera_common::structs::primitives::Cursor;
use tethera_common::structs::client::ServerRow;
use tethera_common::structs::ids::ServerId;
use tethera_common::structs::link::{Link, LinkKind};

/// The remembered list, painted before anything is dialled.
///
/// Every link is `Unknown` rather than `Offline`: nothing has been measured yet,
/// and painting `Offline` would show every machine as dead for the second or two
/// before its answer arrives.
#[tauri::command]
pub(crate) async fn list_servers(state: State<'_, AppState>) -> Result<Vec<ServerRow>, String> {
    Ok(state
        .book()
        .entries()
        .into_iter()
        .map(|entry| ServerRow {
            entry,
            link: Link {
                kind: LinkKind::Unknown,
                rtt_ms: None,
            },
            refusal: None,
        })
        .collect())
}

#[tauri::command]
pub(crate) async fn sweep_servers(state: State<'_, AppState>) -> Result<Vec<ServerRow>, String> {
    let rows = Sweep::run(
        state.endpoint(),
        state.book().entries(),
        state.client(),
        SweepBudget::new(),
    )
    .await;

    // The one sweep after a resume is the answer to whether the resume worked.
    // Every other sweep is the list doing its job every five seconds, and
    // logging those would rotate this one away.
    if state.took_resume() {
        log::info!("sweep after resume: {}", Sweep::summary(&rows));
    }

    // Only a machine that answered is written back. A row that timed out carries
    // the entry unchanged, so persisting it would rewrite the file on every
    // sweep for no change.
    for row in &rows {
        if row.entry.last_seen_at.is_some() {
            state
                .book()
                .upsert(row.entry.clone())
                .map_err(|error| error.to_string())?;
        }
    }

    Ok(rows)
}

#[tauri::command]
pub(crate) async fn forget_server(state: State<'_, AppState>, id: String) -> Result<bool, String> {
    let id = ServerId::parse(&id).ok_or_else(|| format!("{id} is not a server id"))?;

    state.book().forget(&id).map_err(|error| error.to_string())
}

/// One machine's conversations, paged.
///
/// A fresh dial rather than the five the sweep took: this screen is the index,
/// and capping it at what a list row can show would make paging pointless.
#[tauri::command]
pub(crate) async fn list_conversations(
    state: State<'_, AppState>,
    id: String,
    before: Option<String>,
    limit: u16,
) -> Result<Page<Conversation>, String> {
    let id = ServerId::parse(&id).ok_or_else(|| format!("{id} is not a server id"))?;
    let connection = state.connect(&id).await?;

    let payload = Rpc::request(
        &connection,
        Request::ListConversations {
            filter: ConversationFilter::All,
            before: before.map(Cursor::from),
            limit,
        },
    )
    .await
    .map_err(|error| error.to_string())?;

    match payload {
        Payload::Conversations(page) => Ok(page),
        other => Err(format!("the machine answered with {other:?}")),
    }
}
