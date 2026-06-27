use tauri::State;

use nexus_core::{
    error::AppResult,
    services::app_config::{
        AgentDisplayPreferences, OpenCodeGoConnectionParams, ProviderConnectionParams,
        ProviderDisplayPreferences,
    },
};

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

#[tauri::command]
pub fn get_provider_connection_params(
    state: State<'_, AppState>,
    provider_id: String,
) -> AppResult<ProviderConnectionParams> {
    state
        .app_config
        .get_provider_connection_params(&provider_id)
}

#[tauri::command]
pub fn set_provider_connection_params(
    state: State<'_, AppState>,
    provider_id: String,
    params: ProviderConnectionParams,
) -> AppResult<()> {
    state
        .app_config
        .set_provider_connection_params(&provider_id, &params)
}

#[tauri::command]
pub fn get_disabled_agents(state: State<'_, AppState>) -> AppResult<AgentDisplayPreferences> {
    state.app_config.get_agent_display_preferences()
}

#[tauri::command]
pub fn set_disabled_agents(
    state: State<'_, AppState>,
    preferences: AgentDisplayPreferences,
) -> AppResult<AgentDisplayPreferences> {
    state.app_config.set_agent_display_preferences(&preferences)
}

#[tauri::command]
pub fn get_provider_display_preferences(
    state: State<'_, AppState>,
) -> AppResult<ProviderDisplayPreferences> {
    state.app_config.get_provider_display_preferences()
}

#[tauri::command]
pub fn set_provider_display_preferences(
    state: State<'_, AppState>,
    preferences: ProviderDisplayPreferences,
) -> AppResult<ProviderDisplayPreferences> {
    state
        .app_config
        .set_provider_display_preferences(&preferences)
}
