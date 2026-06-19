use tauri::State;

use nexus_core::{error::AppResult, services::provider_quota::ProviderQuotaSnapshot};

use crate::store::AppState;

#[tauri::command]
pub async fn get_provider_quota(
    state: State<'_, AppState>,
    provider_id: String,
) -> AppResult<ProviderQuotaSnapshot> {
    state.provider_quota.get_provider_quota(&provider_id).await
}
