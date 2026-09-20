#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod elevate;
mod known_folders;
mod payload;
mod registry;
mod shortcuts;
mod state;
mod uninstall;

use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::get_initial_state,
            commands::pick_install_dir,
            commands::needs_elevation,
            commands::relaunch_elevated,
            commands::start_install,
            commands::launch_app,
            commands::open_install_folder,
            commands::open_url,
            commands::start_uninstall,
            commands::window_minimize,
            commands::window_close,
        ])
        .setup(|app| {
            let window = app.get_webview_window("main").unwrap();
            #[cfg(target_os = "windows")]
            {
                let _ = window_vibrancy::apply_mica(&window, None);
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running the installer");
}
