use tauri::State;

use nexus_core::error::AppResult;

use crate::store::AppState;

#[tauri::command]
pub fn get_copilot_github_token(state: State<'_, AppState>) -> AppResult<Option<String>> {
    state.app_config.get_copilot_github_token()
}

#[tauri::command]
pub fn set_copilot_github_token(state: State<'_, AppState>, token: String) -> AppResult<()> {
    state.app_config.set_copilot_github_token(&token)
}
