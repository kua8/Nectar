// Crate entry point. Windows logic lives in main_windows.rs, Linux in linux/.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![cfg_attr(not(windows), allow(dead_code))]

// Shared between platforms
mod types;
mod caldav;
mod calendar_sync;
mod ics;
mod i18n;

// Windows only
#[cfg(windows)] mod startup;
#[cfg(windows)] mod state;
#[cfg(windows)] mod utils;
#[cfg(windows)] mod capture;
#[cfg(windows)] mod windows_accent;
#[cfg(windows)] mod services;
#[cfg(windows)] mod commands;
#[cfg(windows)] mod monitors;
#[cfg(windows)] mod updater;
#[cfg(windows)] mod uninstall_registry;
#[cfg(windows)] mod native_uninstall;
#[cfg(windows)] mod diagnostics;
#[cfg(windows)] mod taskbar_pins;
#[cfg(windows)] mod tray_icons;
#[cfg(windows)] mod active_look;
#[cfg(windows)] mod attention;
#[cfg(windows)] mod launchers;
#[cfg(windows)] mod hotkeys;
#[cfg(windows)] mod connections;
#[cfg(windows)] mod audio_devices;
#[cfg(windows)] mod shelf;
#[cfg(windows)] mod agent_api;
#[cfg(windows)] mod memory_saver;
#[cfg(windows)] mod webview_args;
#[cfg(windows)] mod on_demand_windows;
#[cfg(windows)] mod wallpaper;
#[cfg(windows)] mod live_preview;

// Linux backend
#[cfg(target_os = "linux")] #[path = "linux/state.rs"] mod state;
#[cfg(target_os = "linux")] #[path = "linux/utils.rs"] mod utils;
#[cfg(target_os = "linux")] #[path = "linux/diagnostics.rs"] mod diagnostics;
#[cfg(target_os = "linux")] #[path = "linux/x11.rs"] mod x11;
#[cfg(target_os = "linux")] #[path = "linux/desktop.rs"] mod desktop;
#[cfg(target_os = "linux")] #[path = "linux/system.rs"] mod system;
#[cfg(target_os = "linux")] #[path = "linux/media.rs"] mod media;
#[cfg(target_os = "linux")] #[path = "linux/panel.rs"] mod panel;
#[cfg(target_os = "linux")] #[path = "linux/services.rs"] mod services;
#[cfg(target_os = "linux")] #[path = "linux/commands.rs"] mod commands;
#[cfg(target_os = "linux")] #[path = "linux/app.rs"] mod app;

#[cfg(windows)]
include!("main_windows.rs");

#[cfg(target_os = "linux")]
fn main() {
    app::run();
}
