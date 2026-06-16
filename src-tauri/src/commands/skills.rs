use tauri::State;

use crate::{
    error::AppResult,
    services::skills::{SetSkillTargetInput, Skill},
    store::AppState,
};

#[tauri::command]
pub fn list_skills(state: State<'_, AppState>) -> AppResult<Vec<Skill>> {
    state.skills.list_skills()
}

#[tauri::command]
pub fn scan_skills(state: State<'_, AppState>) -> AppResult<Vec<Skill>> {
    state.skills.scan_skills()
}

#[tauri::command]
pub fn set_skill_target(
    state: State<'_, AppState>,
    input: SetSkillTargetInput,
) -> AppResult<Skill> {
    state.skills.set_skill_target(input)
}

#[tauri::command]
pub fn set_skill_disabled(
    state: State<'_, AppState>,
    id: String,
    disabled: bool,
) -> AppResult<Skill> {
    state.skills.set_skill_disabled(id, disabled)
}

#[tauri::command]
pub fn open_skill_source(state: State<'_, AppState>, id: String) -> AppResult<()> {
    state.skills.open_skill_source(id)
}

#[tauri::command]
pub fn reveal_skill_path(state: State<'_, AppState>, id: String) -> AppResult<()> {
    state.skills.reveal_skill_path(id)
}
