use tauri::State;

use nexus_core::{
    error::AppResult,
    services::skills::{
        MoveSkillSourceInput, ProjectCustomSkillIntent, ProjectCustomSkillMutationResult,
        SetSkillTargetInput, SkillRow,
    },
};

use crate::store::AppState;

#[tauri::command]
pub fn list_skills(state: State<'_, AppState>) -> AppResult<Vec<SkillRow>> {
    state.skills.list_skills()
}

#[tauri::command]
pub fn scan_skills(state: State<'_, AppState>) -> AppResult<Vec<SkillRow>> {
    state.skills.scan_skills()
}

#[tauri::command]
pub fn set_skill_target(
    state: State<'_, AppState>,
    input: SetSkillTargetInput,
) -> AppResult<Vec<SkillRow>> {
    state.skills.set_skill_target(input)
}

#[tauri::command]
pub fn move_skill_source(
    state: State<'_, AppState>,
    input: MoveSkillSourceInput,
) -> AppResult<Vec<SkillRow>> {
    state.skills.move_skill_source(input)
}

#[tauri::command]
pub fn set_skill_disabled(
    state: State<'_, AppState>,
    id: String,
    disabled: bool,
) -> AppResult<Vec<SkillRow>> {
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

#[tauri::command]
pub fn apply_project_custom_skill_intent(
    state: State<'_, AppState>,
    intent: ProjectCustomSkillIntent,
) -> AppResult<ProjectCustomSkillMutationResult> {
    state.skills.apply_project_custom_skill_intent(intent)
}
