use crate::state::AppState;
use tauri::State;

/// Told to Rust when the webview comes back to the foreground.
///
/// `hidden` is how long it was away, in milliseconds. It is passed rather than
/// measured here because only the webview knows: this side was frozen for the
/// whole interval, and the duration is the variable that decides whether
/// anything on the transport survived.
#[tauri::command]
pub(crate) async fn resumed(state: State<'_, AppState>, hidden: u64) -> Result<(), String> {
    state.resumed(hidden).await;

    Ok(())
}
