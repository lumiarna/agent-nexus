use tauri::AppHandle;

use crate::tray::{self, TrayEntry};

/// Reconcile the Windows-taskbar tray icons to the desired set. The front end
/// recomputes this whenever quota, the tray metric, or tray visibility change.
#[tauri::command]
pub fn sync_tray(app: AppHandle, entries: Vec<TrayEntry>) -> Result<(), String> {
    tray::sync_tray(&app, entries).map_err(|error| error.to_string())
}
