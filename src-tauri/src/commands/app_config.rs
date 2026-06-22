use tauri::State;

use nexus_core::{error::AppResult, services::app_config::OpenCodeGoConnectionParams};

use crate::store::AppState;

#[tauri::command]
pub fn get_copilot_github_token(state: State<'_, AppState>) -> AppResult<Option<String>> {
    state.app_config.get_copilot_github_token()
}

#[tauri::command]
pub fn set_copilot_github_token(state: State<'_, AppState>, token: String) -> AppResult<()> {
    state.app_config.set_copilot_github_token(&token)
}

#[tauri::command]
pub fn get_opencode_go_connection_params(
    state: State<'_, AppState>,
) -> AppResult<OpenCodeGoConnectionParams> {
    state.app_config.get_opencode_go_connection_params()
}

#[tauri::command]
pub fn set_opencode_go_connection_params(
    state: State<'_, AppState>,
    params: OpenCodeGoConnectionParams,
) -> AppResult<()> {
    state.app_config.set_opencode_go_connection_params(&params)
}
