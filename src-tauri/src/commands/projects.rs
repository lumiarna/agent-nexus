use tauri::State;

use crate::{error::AppResult, services::projects::Project, store::AppState};

#[tauri::command]
pub fn list_projects(state: State<'_, AppState>) -> AppResult<Vec<Project>> {
    state.projects.list_projects()
}

#[tauri::command]
pub fn record_project(state: State<'_, AppState>, path: String) -> AppResult<Project> {
    state.projects.record_project(path)
}
