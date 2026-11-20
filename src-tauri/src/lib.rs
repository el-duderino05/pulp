// SPDX-License-Identifier: MPL-2.0
mod device;
mod error;
mod library_cmds;
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
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            library_cmds::startup,
            library_cmds::create_library,
            library_cmds::open_library,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
