use tauri::State;

use nexus_core::{
    error::AppResult,
    services::{
        provider_quota::{OpenCodeCustomProvider, ProviderQuotaSnapshot},
        provider_trigger::{ProviderScheduleSettings, ProviderTriggerCapability},
    },
};

use crate::store::AppState;

#[tauri::command]
pub async fn get_provider_quota(
    state: State<'_, AppState>,
    provider_id: String,
) -> AppResult<ProviderQuotaSnapshot> {
    state.provider_quota.get_provider_quota(&provider_id).await
}

#[tauri::command]
pub fn list_opencode_custom_providers(
    state: State<'_, AppState>,
) -> AppResult<Vec<OpenCodeCustomProvider>> {
    state.provider_quota.list_opencode_custom_providers()
}

#[tauri::command]
pub fn get_provider_order(state: State<'_, AppState>) -> AppResult<Vec<String>> {
    state.app_config.get_provider_order()
}

#[tauri::command]
pub fn set_provider_order(
    state: State<'_, AppState>,
    provider_ids: Vec<String>,
) -> AppResult<Vec<String>> {
    state.app_config.set_provider_order(&provider_ids)
}

#[tauri::command]
pub fn get_provider_schedule_settings(
    state: State<'_, AppState>,
    provider_id: String,
) -> AppResult<ProviderScheduleSettings> {
    state
        .provider_trigger
        .get_provider_schedule_settings(&provider_id)
}

#[tauri::command]
pub fn set_provider_schedule_settings(
    state: State<'_, AppState>,
    provider_id: String,
    settings: ProviderScheduleSettings,
) -> AppResult<ProviderScheduleSettings> {
    state
        .provider_trigger
        .set_provider_schedule_settings(&provider_id, settings)
}

#[tauri::command]
pub async fn list_provider_trigger_models(
    state: State<'_, AppState>,
    provider_id: String,
) -> AppResult<ProviderTriggerCapability> {
    state
        .provider_trigger
        .list_provider_trigger_models(&provider_id)
        .await
}
