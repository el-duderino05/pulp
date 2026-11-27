// SPDX-License-Identifier: MPL-2.0
mod device;
mod error;
mod library_cmds;
mod project_cmds;
mod state;
mod sync;

use std::sync::Mutex;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let config_path = app.path().app_config_dir()?.join("device.json");
            app.manage(state::AppState {
                device: Mutex::new(device::DeviceConfig::load(&config_path)),
                config_path,
                library: Mutex::new(None),
                projects: Default::default(),
                opening: Default::default(),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            library_cmds::startup,
            library_cmds::create_library,
            library_cmds::open_library,
            library_cmds::list_projects,
            library_cmds::create_project,
            library_cmds::add_external_project,
            library_cmds::forget_external_project,
            library_cmds::trash_project,
            library_cmds::list_library_trash,
            library_cmds::restore_project,
            library_cmds::delete_library_trash_entry,
            library_cmds::empty_library_trash,
            project_cmds::open_project,
            project_cmds::close_project,
            project_cmds::project_tree,
            project_cmds::create_document,
            project_cmds::create_folder,
            project_cmds::rename_node,
            project_cmds::reorder_node,
            project_cmds::move_document,
            project_cmds::trash_node,
            project_cmds::list_project_trash,
            project_cmds::restore_trash_entry,
            project_cmds::delete_trash_entry,
            project_cmds::empty_project_trash,
            project_cmds::read_document,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
