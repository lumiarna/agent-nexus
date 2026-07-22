use nexus_core::{
    error::AppResult,
    services::{
        agent_capabilities::{
            list_agent_capability_surfaces, resolve_agent_config_root, AgentCapabilitySurface,
        },
        system_open::open_path,
    },
};

#[tauri::command]
pub fn list_agent_capabilities() -> AppResult<Vec<AgentCapabilitySurface>> {
    Ok(list_agent_capability_surfaces())
}

#[tauri::command]
pub fn open_agent_config_root(name: String) -> AppResult<()> {
    let config_root = resolve_agent_config_root(&name)?;
    open_path(&config_root)
}
