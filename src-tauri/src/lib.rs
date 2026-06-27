pub mod commands;
pub mod store;

use std::{
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use nexus_core::{
    database::Database,
    services::{
        outbound_request_log::OutboundRequestLogger, provider_trigger::ProviderTriggerService,
        sync::SyncService,
    },
};
use store::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            let db = Database::open(app_data_dir.join("agent-nexus.sqlite3"))?;
            let request_logger = OutboundRequestLogger::from_app_data_dir(&app_data_dir)?;
            let state = AppState::new(db, request_logger);
            let scheduler_sync = state.sync.clone();
            let scheduler_provider_trigger = state.provider_trigger.clone();
            app.manage(state);
            start_background_scheduler(scheduler_sync, scheduler_provider_trigger);
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
            commands::projects::delete_project,
            commands::projects::list_projects,
            commands::projects::record_git_base_folder,
            commands::projects::record_project,
            commands::projects::reorder_projects,
            commands::projects::remove_git_base_folder,
            commands::projects::scan_git_base_folder,
            commands::projects::scan_git_base_folders,
            commands::sessions::get_cloud_session,
            commands::sessions::get_local_session,
            commands::sessions::list_cloud_sessions,
            commands::sessions::list_local_sessions,
            commands::sessions::scan_cloud_sessions,
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
            commands::providers::get_provider_schedule_settings,
            commands::providers::get_provider_order,
            commands::providers::list_opencode_custom_providers,
            commands::providers::list_provider_trigger_models,
            commands::providers::run_provider_window_alignment,
            commands::providers::set_provider_order,
            commands::providers::set_provider_schedule_settings,
            commands::app_config::get_copilot_github_token,
            commands::app_config::get_opencode_go_connection_params,
            commands::app_config::get_provider_connection_params,
            commands::app_config::get_provider_display_preferences,
            commands::app_config::set_copilot_github_token,
            commands::app_config::set_opencode_go_connection_params,
            commands::app_config::set_provider_connection_params,
            commands::app_config::set_provider_display_preferences,
            commands::sync::delete_task,
            commands::sync::delete_task_group,
            commands::sync::add_task,
            commands::sync::get_webdav_settings,
            commands::project_symlinks::list_project_symlinks,
            commands::sync::list_task_groups,
            commands::sync::list_session_backups,
            commands::sync::run_task,
            commands::sync::save_webdav_settings,
            commands::sync::test_webdav_connection,
            commands::sync::update_task_schedule,
            commands::sync::update_group_schedule,
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

fn start_background_scheduler(sync: SyncService, provider_trigger: ProviderTriggerService) {
    thread::spawn(move || loop {
        let now = current_epoch_seconds();
        if let Err(error) = tauri::async_runtime::block_on(sync.run_due_scheduled_tasks(now)) {
            eprintln!("scheduled sync task runner failed: {error}");
        }
        if let Err(error) =
            tauri::async_runtime::block_on(provider_trigger.run_due_window_alignment(now))
        {
            eprintln!("provider window alignment runner failed: {error}");
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
