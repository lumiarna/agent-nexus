use tauri::State;

use nexus_core::{error::AppResult, services::sessions::Session};

use crate::store::AppState;

#[tauri::command]
pub fn list_local_sessions(state: State<'_, AppState>) -> AppResult<Vec<Session>> {
    state.sessions.list_local_sessions()
}

#[tauri::command]
pub fn get_local_session(state: State<'_, AppState>, id: String) -> AppResult<Session> {
    state.sessions.get_local_session(id)
}

#[tauri::command]
pub fn scan_local_sessions(state: State<'_, AppState>) -> AppResult<Vec<Session>> {
    state.sessions.scan_local_sessions()
}
