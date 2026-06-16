use tauri::State;

use crate::{
    error::AppResult,
    services::sync::{CreateTaskGroupInput, ProjectSymlink, TaskGroup},
    store::AppState,
};

#[tauri::command]
pub fn list_task_groups(state: State<'_, AppState>) -> AppResult<Vec<TaskGroup>> {
    state.sync.list_task_groups()
}

#[tauri::command]
pub fn create_task_group(
    state: State<'_, AppState>,
    input: CreateTaskGroupInput,
) -> AppResult<TaskGroup> {
    state.sync.create_task_group(input)
}

#[tauri::command]
pub fn delete_task(state: State<'_, AppState>, id: String) -> AppResult<()> {
    state.sync.delete_task(id)
}

#[tauri::command]
pub fn list_project_symlinks(state: State<'_, AppState>) -> AppResult<Vec<ProjectSymlink>> {
    state.sync.list_project_symlinks()
}

#[tauri::command]
pub fn delete_project_symlink(state: State<'_, AppState>, target_path: String) -> AppResult<()> {
    state.sync.delete_project_symlink(target_path)
}
