// Declared public rather than re-exported. `#[tauri::command]` generates hidden
// items beside each function, and `generate_handler!` needs them; a `pub use` of
// the function alone leaves them behind and the macro fails to resolve.
pub(crate) mod assets;
pub(crate) mod conversation;
pub(crate) mod pairing;
pub(crate) mod servers;
pub(crate) mod settings;
pub(crate) mod terminal;
pub(crate) mod sessions;

use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub(crate) async fn app_version(state: State<'_, AppState>) -> Result<String, String> {
    Ok(state.version().to_string())
}
