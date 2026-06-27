use tauri::State;

use nexus_core::{
    error::AppResult,
    services::sync::{
        CreateTaskGroupInput, CreateTaskInput, SessionBackup, Task, TaskGroup, WebdavSettings,
        WebdavSettingsInput,
    },
};

use crate::store::AppState;

#[tauri::command]
pub fn get_webdav_settings(state: State<'_, AppState>) -> AppResult<WebdavSettings> {
    state.sync.get_webdav_settings()
}

#[tauri::command]
pub fn save_webdav_settings(
    state: State<'_, AppState>,
    input: WebdavSettingsInput,
) -> AppResult<WebdavSettings> {
    state.sync.save_webdav_settings(input)
}

#[tauri::command]
pub async fn test_webdav_connection(
    state: State<'_, AppState>,
    input: WebdavSettingsInput,
) -> AppResult<()> {
    state.sync.test_webdav_connection(input).await
}

#[tauri::command]
pub fn list_task_groups(state: State<'_, AppState>) -> AppResult<Vec<TaskGroup>> {
    state.sync.list_task_groups()
}

#[tauri::command]
pub fn list_session_backups(state: State<'_, AppState>) -> AppResult<Vec<SessionBackup>> {
    state.sync.list_session_backups()
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
pub fn delete_task_group(state: State<'_, AppState>, id: String) -> AppResult<()> {
    state.sync.delete_task_group(id)
}

#[tauri::command]
pub fn add_task(
    state: State<'_, AppState>,
    group_id: String,
    task: CreateTaskInput,
) -> AppResult<TaskGroup> {
    state.sync.add_task(group_id, task)
}

#[tauri::command]
pub fn update_task_schedule(
    state: State<'_, AppState>,
    id: String,
    schedule: String,
) -> AppResult<Task> {
    state.sync.update_task_schedule(id, schedule)
}

#[tauri::command]
pub fn update_group_schedule(
    state: State<'_, AppState>,
    group_id: String,
    schedule: String,
) -> AppResult<()> {
    state.sync.update_group_schedule(group_id, schedule)
}

#[tauri::command]
pub fn reorder_task_groups(
    state: State<'_, AppState>,
    group_ids: Vec<String>,
) -> AppResult<Vec<TaskGroup>> {
    state.sync.reorder_task_groups(group_ids)
}

#[tauri::command]
pub fn reorder_tasks(
    state: State<'_, AppState>,
    group_id: String,
    task_ids: Vec<String>,
) -> AppResult<TaskGroup> {
    state.sync.reorder_tasks(group_id, task_ids)
}

#[tauri::command]
pub async fn run_task(state: State<'_, AppState>, id: String) -> AppResult<Task> {
    state.sync.run_task(id).await
}
