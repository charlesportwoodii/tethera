use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub(crate) async fn app_version(state: State<'_, AppState>) -> Result<String, String> {
    Ok(state.version().to_string())
}
