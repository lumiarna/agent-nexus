pub mod commands;
pub mod store;

use std::{
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use nexus_core::{database::Database, services::sync::SyncService};
use store::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            let db = Database::open(app_data_dir.join("agent-nexus.sqlite3"))?;
            let state = AppState::new(db);
            let scheduler_sync = state.sync.clone();
            app.manage(state);
            start_sync_scheduler(scheduler_sync);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::agent_capabilities::list_agent_capabilities,
            commands::app::get_desktop_health,
            commands::app::get_platform,
            commands::projects::list_git_base_folders,
            commands::prompts::list_prompts,
            commands::prompts::open_prompt_source,
            commands::prompts::reveal_prompt_path,
            commands::prompts::scan_prompts,
            commands::prompts::set_prompt_target,
            commands::projects::list_projects,
            commands::projects::record_git_base_folder,
            commands::projects::record_project,
            commands::projects::reorder_projects,
            commands::projects::remove_git_base_folder,
            commands::projects::scan_git_base_folder,
            commands::projects::scan_git_base_folders,
            commands::sessions::get_local_session,
            commands::sessions::list_local_sessions,
            commands::sessions::scan_local_sessions,
            commands::skills::list_skills,
            commands::skills::open_skill_source,
            commands::skills::reveal_skill_path,
            commands::skills::scan_skills,
            commands::skills::set_skill_disabled,
            commands::skills::set_skill_target,
            commands::sync::create_task_group,
            commands::project_symlinks::delete_project_symlink,
            commands::providers::get_provider_quota,
            commands::app_config::get_copilot_github_token,
            commands::app_config::get_opencode_go_connection_params,
            commands::app_config::set_copilot_github_token,
            commands::app_config::set_opencode_go_connection_params,
            commands::sync::delete_task,
            commands::sync::delete_task_group,
            commands::sync::add_task,
            commands::sync::get_webdav_settings,
            commands::project_symlinks::list_project_symlinks,
            commands::sync::list_task_groups,
            commands::sync::run_task,
            commands::sync::save_webdav_settings,
            commands::sync::test_webdav_connection,
            commands::sync::update_task_schedule,
        ])
        .on_window_event(|window, event| {
            if window.label() == "main"
                && matches!(event, tauri::WindowEvent::CloseRequested { .. })
            {
                window.app_handle().exit(0);
            }
        })
        .run(tauri::generate_context!())
        .expect("failed to run Agent Nexus");
}

fn start_sync_scheduler(sync: SyncService) {
    thread::spawn(move || loop {
        let now = current_epoch_seconds();
        if let Err(error) = tauri::async_runtime::block_on(sync.run_due_scheduled_tasks(now)) {
            eprintln!("scheduled sync task runner failed: {error}");
        }

        let now = current_epoch_seconds();
        let sleep_secs = 60 - now.rem_euclid(60) as u64;
        thread::sleep(Duration::from_secs(sleep_secs));
    });
}

fn current_epoch_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is before Unix epoch")
        .as_secs() as i64
}
