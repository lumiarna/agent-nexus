use nexus_core::{
    error::AppResult,
    services::agent_capabilities::{list_agent_capability_surfaces, AgentCapabilitySurface},
};

#[tauri::command]
pub fn list_agent_capabilities() -> AppResult<Vec<AgentCapabilitySurface>> {
    Ok(list_agent_capability_surfaces())
}
