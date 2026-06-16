use tauri::State;

use crate::{error::AppResult, services::sync::ProjectSymlink, store::AppState};

#[tauri::command]
pub fn list_project_symlinks(state: State<'_, AppState>) -> AppResult<Vec<ProjectSymlink>> {
    state.sync.list_project_symlinks()
}
