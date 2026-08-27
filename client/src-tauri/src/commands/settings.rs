use crate::state::AppState;
use tauri::State;
use tethera_common::structs::client::Preferences;

#[tauri::command]
pub(crate) async fn read_preferences(state: State<'_, AppState>) -> Result<Preferences, String> {
    Ok(state.settings().preferences())
}

/// Turns the launch lock on or off.
///
/// Authenticating first is the caller's job and is not optional: without it
/// anybody holding the unlocked phone could switch the lock off, which is the
/// one person it is meant to stop.
#[tauri::command]
pub(crate) async fn set_biometric_lock(
    state: State<'_, AppState>,
    on: bool,
) -> Result<Preferences, String> {
    state
        .settings()
        .set_biometric_lock(on)
        .map_err(|error| error.to_string())
}

/// Whether a machine can be reached right now.
///
/// Answered from the same flag `connect` consults, rather than from a copy the
/// screen keeps, so the screen and the gate can never disagree.
#[tauri::command]
pub(crate) async fn is_unlocked(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(state.settings().unlocked())
}

/// Opens the door after the platform has said who this is.
///
/// The authentication itself happens in the webview, through the biometric
/// plugin, and this only records the result. That is the honest shape of a
/// launch lock and its limit is worth stating plainly: it stops somebody holding
/// an unlocked phone, and it does not stop somebody who can run code in this
/// process. Making it stop the second thing means the identity key itself has to
/// require authentication before it will sign, which is a different feature.
#[tauri::command]
pub(crate) async fn unlock(state: State<'_, AppState>) -> Result<(), String> {
    state.settings().unlock();

    Ok(())
}

/// Closes it again, which is what leaving the app does.
#[tauri::command]
pub(crate) async fn lock(state: State<'_, AppState>) -> Result<(), String> {
    state.settings().lock();

    Ok(())
}
