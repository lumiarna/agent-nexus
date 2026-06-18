use tauri::State;

use nexus_core::{error::AppResult, services::project_symlinks::ProjectSymlink};

use crate::store::AppState;

#[tauri::command]
pub fn list_project_symlinks(state: State<'_, AppState>) -> AppResult<Vec<ProjectSymlink>> {
    state.project_symlinks.list_project_symlinks()
}

#[tauri::command]
pub fn delete_project_symlink(state: State<'_, AppState>, target_path: String) -> AppResult<()> {
    state.project_symlinks.delete_project_symlink(target_path)
}
