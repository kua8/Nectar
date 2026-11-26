//! Linux entry point: single instance, plugins, command registration, window
//! setup and background workers.

use std::io::{Read, Write};
use std::sync::atomic::Ordering;
use tauri::Manager;

use crate::commands::*;

fn socket_path() -> std::path::PathBuf {
    let dir = std::env::var_os("XDG_RUNTIME_DIR").map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir);
    dir.join("nectar.sock")
}

/// Returns false when another Nectar is running; it is asked to open its settings.
fn claim_single_instance() -> Option<std::os::unix::net::UnixListener> {
    use std::os::unix::net::{UnixListener, UnixStream};
    let path = socket_path();
    // After a restart the old process can hold the socket for a moment, so wait it out.
    let attempts = if std::env::var_os("NECTAR_WAS_RESTARTED").is_some() { 20 } else { 1 };
    for attempt in 0..attempts {
        match UnixStream::connect(&path) {
            Ok(mut stream) if attempt + 1 == attempts => {
                let _ = stream.write_all(b"open-settings\n");
                return None;
            }
            Ok(_) => std::thread::sleep(std::time::Duration::from_millis(200)),
            Err(_) => break,
        }
    }
    let _ = std::fs::remove_file(&path);
    UnixListener::bind(&path).ok()
}

/// Dock, notch and overlay behave like panels: on top, on every workspace, never in
/// alt-tab, and they don't take focus when clicked.
pub fn make_panel_window(win: &tauri::WebviewWindow) {
    use gtk::prelude::*;
    if let Ok(gw) = win.gtk_window() {
        gw.set_type_hint(gtk::gdk::WindowTypeHint::Dock);
        gw.set_accept_focus(false);
        gw.set_skip_taskbar_hint(true);
        gw.set_skip_pager_hint(true);
        gw.set_keep_above(true);
        gw.stick();
    }
}

/// SIGTERM/SIGINT/SIGHUP (logout, `kill`, Ctrl+C) restore the panel before exiting.
/// The signals are blocked in every thread and handled by one waiting thread, so the
/// cleanup runs as normal code instead of inside a signal handler.
fn handle_exit_signals() {
    unsafe {
        let mut set: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&mut set);
        for sig in [libc::SIGTERM, libc::SIGINT, libc::SIGHUP] {
            libc::sigaddset(&mut set, sig);
        }
        if libc::pthread_sigmask(libc::SIG_BLOCK, &set, std::ptr::null_mut()) != 0 {
            return;
        }
        let set_copy = set;
        std::thread::spawn(move || {
            let mut sig = 0;
            if libc::sigwait(&set_copy, &mut sig) == 0 {
                crate::diagnostics::log(&format!("signal {sig}: restoring the panel and exiting"));
                crate::state::SHUTTING_DOWN.store(true, Ordering::Relaxed);
                crate::panel::set_visible(true);
                if let Some(xid) = crate::x11::find_own_window("Nectar Dock") {
                    crate::x11::clear_strut(xid);
                }
                let _ = std::fs::remove_file(socket_path());
                std::process::exit(0);
            }
        });
    }
}

pub fn run() {
    // WebKitGTK's DMA-BUF renderer shows blank or black transparent windows on many
    // NVIDIA and hybrid setups.
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }
    // Window management uses X11/EWMH; under Wayland run through XWayland.
    if std::env::var("XDG_SESSION_TYPE").is_ok_and(|s| s == "wayland") && std::env::var_os("GDK_BACKEND").is_none() {
        std::env::set_var("GDK_BACKEND", "x11");
    }

    let restarting = std::env::var_os("NECTAR_RESTARTING").is_some();
    if restarting {
        std::env::remove_var("NECTAR_RESTARTING");
        std::env::set_var("NECTAR_WAS_RESTARTED", "1");
    }
    let listener = claim_single_instance();
    std::env::remove_var("NECTAR_WAS_RESTARTED");
    let Some(listener) = listener else { return };
    // Before any other thread starts, so they all inherit the blocked mask.
    handle_exit_signals();

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_drag::init())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, Some(vec![])))
        .invoke_handler(tauri::generate_handler![
            hide_native_osd,
            setup_shelf_drop,
            shelf_thumbnail,
            shelf_text_preview,
            shelf_open,
            shelf_prune,
            shelf_drag_icon,
            shelf_text_file,
            pointer_over_notch,
            open_settings_window,
            on_demand_window_ready,
            open_calendar_window,
            open_tray_window,
            open_tray_at_cursor,
            open_tray_from_button,
            show_desktop,
            open_wifi_settings,
            open_sound_settings,
            open_notification_center,
            set_ignore_cursor_events,
            set_window_height,
            resize_settings_window,
            hide_overlay,
            set_splash_fullscreen,
            sync_overlay_position,
            select_media_session,
            media_play_pause,
            media_next,
            media_previous,
            media_seek,
            get_app_media_sessions,
            app_media_control,
            open_media_source_app,
            init_dock,
            toggle_dock,
            change_dock_mode,
            change_notch_mode,
            sync_appbar,
            open_app,
            launch_new_instance,
            update_dock_rect,
            update_notch_rect,
            focus_agent_window,
            agent_answer,
            pick_agent_sound,
            pick_program,
            reset_agent_sound,
            agent_sound_data,
            agent_sound_custom,
            set_dock_hovered,
            set_notch_hovered,
            get_media_info,
            set_notch_hidden,
            set_dock_dragging,
            get_active_windows,
            get_autostart,
            set_autostart,
            get_app_icon,
            get_installed_apps,
            save_pinned_apps,
            load_pinned_apps,
            clear_icon_cache,
            set_custom_icon,
            remove_custom_icon,
            get_custom_icons,
            set_menu_open,
            set_window_keyboard_focus,
            get_drives,
            open_dock_extra,
            get_audio_devices,
            set_default_audio_device,
            get_lan_status,
            open_ethernet_settings,
            wifi_scan,
            wifi_connect,
            wifi_disconnect,
            wifi_forget,
            bluetooth_devices,
            bluetooth_unpair,
            focus_window,
            bring_window_forward,
            focus_app_windows,
            close_window,
            end_task,
            quit_nectar,
            restart_nectar,
            uninstall_nectar,
            get_diagnostics,
            open_log_folder,
            set_hitbox_logging,
            get_hitbox_logging,
            get_volume,
            get_volume_state,
            get_brightness,
            set_volume,
            get_audio_sessions,
            set_app_volume,
            set_app_mute,
            set_volume_mixer_rect,
            clear_volume_mixer_rect,
            save_setting,
            load_settings,
            reset_settings,
            capture_window_thumbnail,
            set_live_thumbnails,
            clear_live_thumbnails,
            set_live_thumbnails_opacity,
            is_dwm_composition_enabled,
            get_wifi_status,
            set_wifi_state,
            get_bluetooth_state,
            set_bluetooth_state,
            open_bluetooth_settings,
            open_airplane_mode_settings,
            set_brightness,
            get_battery_saver_state,
            has_battery,
            open_battery_saver_settings,
            get_system_accent_color,
            get_attention_windows,
            get_wallpaper_backdrop,
            sync_windows_accent,
            restore_windows_accent,
            windows_accent_backup_exists,
            is_backdrop_covered,
            get_monitors,
            get_dock_mode_for_window,
            get_notch_mode_for_window,
            set_dock_mode_for_window,
            set_notch_mode_for_window,
            get_system_metrics,
            get_cpu_usage,
            get_ram_usage,
            get_disk_space,
            get_network_speed,
            get_ping_ms,
            get_running_processes,
            kill_process,
            export_settings,
            import_settings,
            export_settings_dialog,
            import_settings_dialog,
            check_for_updates,
            install_update,
            get_update_state,
            crate::caldav::caldav_connect,
            crate::caldav::caldav_disconnect,
            crate::calendar_sync::caldav_sync_now,
            crate::calendar_sync::caldav_get_cache,
            crate::ics::ics_add_link,
            crate::ics::ics_remove_link,
            crate::ics::ics_detect
        ])
        .setup(move |app| {
            let handle = app.handle().clone();
            if let Ok(dir) = app.path().app_log_dir() {
                crate::diagnostics::init(dir);
            }
            crate::diagnostics::log(&format!("Nectar {} starting on Linux", app.package_info().version));
            if !crate::x11::available() {
                crate::diagnostics::log("no X11 display: window list, focus and click-through need X11 or XWayland");
            }
            crate::utils::init_settings_cache(&handle);
            if let Ok(dir) = app.path().app_config_dir() {
                crate::panel::init(dir);
            }
            // A crash leaves the panel on auto-hide; put it back before the dock hides it again.
            if crate::panel::restore_after_crash() || restarting {
                crate::state::RELAUNCHED.store(true, Ordering::Relaxed);
            }

            for label in ["main", "dock", "overlay"] {
                if let Some(w) = app.get_webview_window(label) {
                    make_panel_window(&w);
                    crate::services::wire_window_events(&handle, &w);
                }
            }
            crate::services::position_notch(&handle);
            crate::services::sync_overlay(&handle);

            crate::services::start_pointer_loop(handle.clone());
            crate::services::start_window_watcher(handle.clone());
            crate::services::start_volume_watcher(handle.clone());
            crate::services::start_brightness_watcher(handle.clone());
            crate::services::start_settings_watcher(handle.clone());
            crate::media::start_watcher(handle.clone());
            crate::calendar_sync::start_background_sync(handle.clone());
            // Warm the desktop entry index so the first dock refresh is quick.
            std::thread::spawn(crate::desktop::entries);

            // Second instance -> open settings.
            {
                let h = handle.clone();
                std::thread::spawn(move || {
                    for mut stream in listener.incoming().flatten() {
                        let mut buf = [0u8; 64];
                        let _ = stream.read(&mut buf);
                        crate::commands::open_settings_window(h.clone());
                    }
                });
            }

            // Tray icon.
            {
                use tauri::menu::{Menu, MenuItem};
                use tauri::tray::TrayIconBuilder;
                let make = |id: &str, key: &'static str| -> tauri::Result<MenuItem<tauri::Wry>> {
                    let item = MenuItem::with_id(app, id, crate::i18n::tr(&handle, key), true, None::<&str>)?;
                    crate::i18n::register_tray_item(item.clone(), key);
                    Ok(item)
                };
                let menu = Menu::with_items(
                    app,
                    &[
                        &make("calendar", "dockmenu.openCalendar")?,
                        &make("settings", "dockmenu.openSettings")?,
                        &make("restart", "general.restart")?,
                        &make("quit", "general.quit")?,
                    ],
                )?;
                let ah = handle.clone();
                let mut tray = TrayIconBuilder::new().tooltip("Nectar").menu(&menu).on_menu_event(move |_, event| match event.id().as_ref() {
                    "quit" => crate::commands::restore_and_exit(&ah),
                    "restart" => {
                        crate::state::SHUTTING_DOWN.store(true, Ordering::Relaxed);
                        std::env::set_var("NECTAR_RESTARTING", "1");
                        ah.restart();
                    }
                    "settings" => crate::commands::open_settings_window(ah.clone()),
                    "calendar" => crate::commands::open_calendar_window(ah.clone()),
                    _ => {}
                });
                if let Some(icon) = app.default_window_icon() {
                    tray = tray.icon(icon.clone());
                }
                tray.build(app)?;
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|handle, event| {
        if let tauri::RunEvent::Exit = event {
            crate::panel::set_visible(true);
            if let Some(xid) = crate::x11::find_own_window("Nectar Dock") {
                crate::x11::clear_strut(xid);
            }
            let _ = std::fs::remove_file(socket_path());
            let _ = handle;
        }
    });
}
