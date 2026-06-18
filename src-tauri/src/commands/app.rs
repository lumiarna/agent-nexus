use serde::Serialize;

use nexus_core::error::AppResult;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopHealth {
    pub ok: bool,
    pub app_name: &'static str,
    pub app_version: &'static str,
}

#[tauri::command]
pub fn get_desktop_health() -> AppResult<DesktopHealth> {
    Ok(DesktopHealth {
        ok: true,
        app_name: "Agent Nexus",
        app_version: env!("CARGO_PKG_VERSION"),
    })
}

/// Host OS identifier (`windows` / `macos` / `linux` / ...). Drives platform-only
/// UI affordances such as hiding the Junction action where it is unsupported.
#[tauri::command]
pub fn get_platform() -> &'static str {
    std::env::consts::OS
}
