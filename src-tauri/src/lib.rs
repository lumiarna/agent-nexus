pub mod commands;
pub mod database;
pub mod error;
pub mod services;
pub mod store;

use database::Database;
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
            commands::projects::list_git_base_folders,
            commands::projects::list_projects,
            commands::projects::record_git_base_folder,
            commands::projects::record_project,
            commands::projects::remove_git_base_folder,
            commands::projects::scan_git_base_folder,
            commands::projects::scan_git_base_folders,
            commands::sync::create_task_group,
            commands::sync::delete_project_symlink,
            commands::sync::delete_task,
            commands::sync::list_project_symlinks,
            commands::sync::list_task_groups,
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
