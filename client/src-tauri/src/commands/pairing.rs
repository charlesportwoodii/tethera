use crate::state::AppState;
use tauri::State;
use tethera_client_core::pairing::PairingSession;
use tethera_common::structs::client::{BeginOutcome, PairOutcome};

#[tauri::command]
pub(crate) async fn pair_begin(
    state: State<'_, AppState>,
    uri: String,
) -> Result<BeginOutcome, String> {
    let begun = PairingSession::begin(
        state.endpoint(),
        &uri,
        state.client(),
        state.device_name(),
    )
    .await
    .map_err(|error| error.to_string())?;

    // Re-scanning a machine that already knows this device is not an error, and
    // its dial details may have moved since the last pairing.
    if let BeginOutcome::AlreadyPaired(entry) = &begun.outcome {
        state
            .book()
            .upsert(entry.clone())
            .map_err(|error| error.to_string())?;
    }

    *state.pairing().lock().await = begun.session;

    Ok(begun.outcome)
}

#[tauri::command]
pub(crate) async fn pair_submit(
    state: State<'_, AppState>,
    code: String,
) -> Result<PairOutcome, String> {
    let mut held = state.pairing().lock().await;

    let session = held
        .as_mut()
        .ok_or_else(|| "there is no pairing attempt open".to_string())?;

    let outcome = session.submit(&code).await.map_err(|e| e.to_string())?;

    match &outcome {
        PairOutcome::Paired(entry) => {
            state
                .book()
                .upsert(entry.clone())
                .map_err(|error| error.to_string())?;
            *held = None;
        }
        // The only outcome that leaves the session usable. Everything else has
        // finished the exchange, so the stream is dropped and the machine sees
        // the reset.
        PairOutcome::WrongCode { .. } => {}
        _ => *held = None,
    }

    Ok(outcome)
}

#[tauri::command]
pub(crate) async fn pair_cancel(state: State<'_, AppState>) -> Result<(), String> {
    *state.pairing().lock().await = None;

    Ok(())
}
