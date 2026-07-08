use tauri::State;

use nexus_core::{
    error::AppResult,
    services::skills::{
        SetProjectSkillProjectInput, SetProjectSkillTargetInput, SetSkillTargetInput, Skill,
    },
};

use crate::store::AppState;

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

/// Source-side: propagate a Project custom Skill to (or cancel it from) a
/// target Project. Returns the full skill list so the UI can refetch the
/// incoming target-Project projection rows.
#[tauri::command]
pub fn set_project_skill_project(
    state: State<'_, AppState>,
    input: SetProjectSkillProjectInput,
) -> AppResult<Vec<Skill>> {
    state.skills.set_project_skill_project(input)
}

/// Target-side: toggle a single Agent placement inside an incoming target
/// Project Skill row.
#[tauri::command]
pub fn set_project_skill_target(
    state: State<'_, AppState>,
    input: SetProjectSkillTargetInput,
) -> AppResult<Vec<Skill>> {
    state.skills.set_project_skill_target(input)
}
