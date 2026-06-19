use tauri::State;

use nexus_core::{
    error::AppResult,
    services::prompts::{Prompt, SetPromptTargetInput},
};

use crate::store::AppState;

#[tauri::command]
pub fn list_prompts(state: State<'_, AppState>) -> AppResult<Vec<Prompt>> {
    state.prompts.list_prompts()
}

#[tauri::command]
pub fn scan_prompts(state: State<'_, AppState>) -> AppResult<Vec<Prompt>> {
    state.prompts.scan_prompts()
}

#[tauri::command]
pub fn set_prompt_target(
    state: State<'_, AppState>,
    input: SetPromptTargetInput,
) -> AppResult<Prompt> {
    state.prompts.set_prompt_target(input)
}

#[tauri::command]
pub fn open_prompt_source(state: State<'_, AppState>, id: String) -> AppResult<()> {
    state.prompts.open_prompt_source(id)
}

#[tauri::command]
pub fn reveal_prompt_path(state: State<'_, AppState>, id: String) -> AppResult<()> {
    state.prompts.reveal_prompt_path(id)
}
