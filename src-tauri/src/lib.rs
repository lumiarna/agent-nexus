pub mod commands;
pub mod store;

use nexus_core::database::Database;
use store::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            let db = Database::open(app_data_dir.join("agent-nexus.sqlite3"))?;
            app.manage(AppState::new(db));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::get_desktop_health,
            commands::app::get_platform,
            commands::projects::list_git_base_folders,
            commands::projects::list_projects,
            commands::projects::record_git_base_folder,
            commands::projects::record_project,
            commands::projects::remove_git_base_folder,
            commands::projects::scan_git_base_folder,
            commands::projects::scan_git_base_folders,
            commands::skills::list_skills,
            commands::skills::open_skill_source,
            commands::skills::reveal_skill_path,
            commands::skills::scan_skills,
            commands::skills::set_skill_disabled,
            commands::skills::set_skill_target,
            commands::sync::create_task_group,
            commands::sync::delete_project_symlink,
            commands::sync::delete_task,
            commands::sync::delete_task_group,
            commands::sync::add_task,
            commands::sync::get_webdav_settings,
            commands::sync::list_project_symlinks,
            commands::sync::list_task_groups,
            commands::sync::run_task,
            commands::sync::save_webdav_settings,
            commands::sync::test_webdav_connection,
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
