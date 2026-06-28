use tauri::State;

use nexus_core::{
    error::AppResult,
    services::projects::{DiscoveredRepo, GitBaseFolder, Project, ProjectDefaults},
};

use crate::store::AppState;

#[tauri::command]
pub fn list_projects(state: State<'_, AppState>) -> AppResult<Vec<Project>> {
    state.projects.list_projects()
}

#[tauri::command]
pub fn record_project(state: State<'_, AppState>, path: String) -> AppResult<Project> {
    state.projects.record_project(path)
}

#[tauri::command]
pub fn delete_project(state: State<'_, AppState>, id: String) -> AppResult<()> {
    state.projects.delete_project(id)
}

#[tauri::command]
pub fn reorder_projects(
    state: State<'_, AppState>,
    project_ids: Vec<String>,
) -> AppResult<Vec<Project>> {
    state.projects.reorder_projects(project_ids)
}

#[tauri::command]
pub fn list_git_base_folders(state: State<'_, AppState>) -> AppResult<Vec<GitBaseFolder>> {
    state.projects.list_git_base_folders()
}

#[tauri::command]
pub fn record_git_base_folder(
    state: State<'_, AppState>,
    path: String,
) -> AppResult<GitBaseFolder> {
    state.projects.record_git_base_folder(path)
}

#[tauri::command]
pub fn remove_git_base_folder(state: State<'_, AppState>, id: String) -> AppResult<()> {
    state.projects.remove_git_base_folder(id)
}

#[tauri::command]
pub fn scan_git_base_folder(
    state: State<'_, AppState>,
    path: String,
) -> AppResult<Vec<DiscoveredRepo>> {
    state.projects.scan_git_base_folder(path)
}

#[tauri::command]
pub fn scan_git_base_folders(state: State<'_, AppState>) -> AppResult<Vec<DiscoveredRepo>> {
    state.projects.scan_git_base_folders()
}

#[tauri::command]
pub fn set_project_custom_skills_dirs(
    state: State<'_, AppState>,
    project_id: String,
    dirs: Vec<String>,
) -> AppResult<Project> {
    state
        .projects
        .set_project_custom_skills_dirs(project_id, dirs)
}

#[tauri::command]
pub fn set_project_extra_prompt_files(
    state: State<'_, AppState>,
    project_id: String,
    files: Vec<String>,
) -> AppResult<Project> {
    state
        .projects
        .set_project_extra_prompt_files(project_id, files)
}

#[tauri::command]
pub fn set_project_sessions_dir(
    state: State<'_, AppState>,
    project_id: String,
    dir: String,
) -> AppResult<Project> {
    state.projects.set_project_sessions_dir(project_id, dir)
}

#[tauri::command]
pub fn get_project_defaults(state: State<'_, AppState>) -> AppResult<ProjectDefaults> {
    state.projects.get_project_defaults()
}

#[tauri::command]
pub fn set_default_custom_skills_dirs(
    state: State<'_, AppState>,
    dirs: Vec<String>,
) -> AppResult<ProjectDefaults> {
    state.projects.set_default_custom_skills_dirs(dirs)
}

#[tauri::command]
pub fn set_default_extra_prompt_files(
    state: State<'_, AppState>,
    files: Vec<String>,
) -> AppResult<ProjectDefaults> {
    state.projects.set_default_extra_prompt_files(files)
}

#[tauri::command]
pub fn set_default_sessions_dir(
    state: State<'_, AppState>,
    dir: String,
) -> AppResult<ProjectDefaults> {
    state.projects.set_default_sessions_dir(dir)
}
