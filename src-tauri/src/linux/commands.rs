//! Tauri commands for the Linux backend. Names and arguments match the Windows
//! backend so the same frontend runs on both.

use crate::state::*;
use crate::types::{AppInfo, AudioSessionInfo, IntRect, MediaInfo, MonitorInfo, VolumeChangeEvent, WifiStatus};
use crate::utils::get_setting_str;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, Manager, Window};
use tauri_plugin_dialog::DialogExt;

// ---------------------------------------------------------------- hit areas

#[tauri::command]
pub async fn set_menu_open(open: bool, rect: Option<IntRect>) {
    MENU_IS_OPEN.store(open, Ordering::Relaxed);
    if let Ok(mut r) = MENU_RECT.lock() {
        *r = rect;
    }
}

#[tauri::command]
pub fn set_window_keyboard_focus(window: Window, enabled: bool) {
    // Panel windows refuse focus by default; typing in the dock search needs it.
    let w = window.clone();
    let _ = window.run_on_main_thread(move || {
        use gtk::prelude::*;
        if let Ok(gw) = w.gtk_window() {
            gw.set_accept_focus(enabled);
            if enabled {
                gw.present();
            }
        }
    });
}

#[tauri::command]
pub async fn set_dock_hovered(hovered: bool) {
    DOCK_HOVERED.store(hovered, Ordering::Relaxed);
}

#[tauri::command]
pub async fn set_notch_hovered(hovered: bool) {
    NOTCH_HOVERED.store(hovered, Ordering::Relaxed);
}

#[tauri::command]
pub async fn set_notch_hidden(hidden: bool) {
    NOTCH_HIDDEN.store(hidden, Ordering::Relaxed);
}

#[tauri::command]
pub async fn set_dock_dragging(dragging: bool) {
    DOCK_IS_DRAGGING.store(dragging, Ordering::Relaxed);
}

#[tauri::command]
pub async fn update_dock_rect(rect: IntRect, expanded: Option<bool>) {
    if expanded == Some(true) {
        if let Ok(mut r) = DOCK_OVERLAP_RECT.lock() {
            *r = Some(rect);
        }
    }
    if let Ok(mut r) = DOCK_RECT.lock() {
        *r = Some(rect);
    }
}

#[tauri::command]
pub async fn update_notch_rect(rect: IntRect) {
    if let Ok(mut r) = NOTCH_RECT.lock() {
        *r = Some(rect);
    }
}

#[tauri::command]
pub fn pointer_over_notch(window: Window) -> bool {
    let Some((cx, cy, _)) = crate::x11::query_pointer() else { return false };
    let Some((wx, wy, _, _)) = crate::services::window_rect(window.label()) else { return false };
    let Some(r) = NOTCH_RECT.lock().ok().and_then(|g| *g) else { return false };
    let scale = window.scale_factor().unwrap_or(1.0);
    let (rx, ry) = (wx + (r.x as f64 * scale) as i32, wy + (r.y as f64 * scale) as i32);
    cx >= rx && cx <= rx + (r.width as f64 * scale) as i32 && cy >= ry && cy <= ry + (r.height as f64 * scale) as i32
}

#[tauri::command]
pub fn set_window_height(window: Window, height: f64) {
    if let (Ok(scale), Ok(size)) = (window.scale_factor(), window.inner_size()) {
        let _ = window.set_size(tauri::LogicalSize::new(size.width as f64 / scale, height));
        if window.label() == "dock" {
            let app = window.app_handle().clone();
            let mode = get_setting_str(&app, "nectar-dock-mode").unwrap_or_else(|| "smart".into());
            crate::services::position_dock(&app, &mode);
        }
    }
}

#[tauri::command]
pub fn resize_settings_window(app: AppHandle, width: f64, height: f64) {
    if let Some(win) = app.get_webview_window("settings") {
        let _ = win.set_size(tauri::LogicalSize::new(width, height));
    }
}

#[tauri::command]
pub fn set_ignore_cursor_events(window: Window, ignore: bool) {
    if window.is_visible().unwrap_or(false) {
        let _ = window.set_ignore_cursor_events(ignore);
    }
}

// ---------------------------------------------------------------- dock / notch

pub async fn wait_for_splash() {
    let waited = std::time::Instant::now();
    while !SPLASH_FINISHED.load(Ordering::Relaxed) && waited.elapsed() < std::time::Duration::from_secs(8) {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}

fn dock_enabled(app: &AppHandle) -> bool {
    get_setting_str(app, "nectar-dock-enabled").unwrap_or_else(|| "true".into()) == "true"
}

#[tauri::command]
pub async fn init_dock(app: AppHandle, mode: String) {
    if !dock_enabled(&app) {
        if let Some(w) = app.get_webview_window("dock") {
            let _ = w.hide();
        }
        return;
    }
    wait_for_splash().await;
    crate::services::position_dock(&app, &mode);
    crate::panel::set_visible(false);
    crate::services::reset_overlap_state(&app);
}

#[tauri::command]
pub async fn toggle_dock(app: AppHandle, enable: bool) {
    if enable {
        let mode = get_setting_str(&app, "nectar-dock-mode").unwrap_or_else(|| "smart".into());
        init_dock(app, mode).await;
    } else {
        if let Some(w) = app.get_webview_window("dock") {
            let _ = w.hide();
        }
        if let Some(xid) = crate::x11::find_own_window("Nectar Dock") {
            crate::x11::clear_strut(xid);
        }
        crate::panel::set_visible(true);
    }
}

#[tauri::command]
pub async fn sync_appbar(app: AppHandle) {
    if dock_enabled(&app) && app.get_webview_window("dock").is_some_and(|w| w.is_visible().unwrap_or(false)) {
        let mode = get_setting_str(&app, "nectar-dock-mode").unwrap_or_else(|| "smart".into());
        crate::services::position_dock(&app, &mode);
    }
    crate::services::sync_overlay(&app);
}

#[tauri::command]
pub async fn change_dock_mode(app: AppHandle, mode: String) {
    if !dock_enabled(&app) {
        return;
    }
    wait_for_splash().await;
    crate::services::position_dock(&app, &mode);
    crate::services::reset_overlap_state(&app);
}

#[tauri::command]
pub async fn change_notch_mode(app: AppHandle, mode: String) {
    let _ = mode;
    wait_for_splash().await;
    crate::services::position_notch(&app);
    crate::services::reset_overlap_state(&app);
}

#[tauri::command]
pub fn get_monitors(app: AppHandle) -> Vec<MonitorInfo> {
    let _ = app;
    crate::services::monitors()
        .iter()
        .enumerate()
        .map(|(i, m)| MonitorInfo {
            id: crate::services::monitor_id(m),
            label: m.name().cloned().unwrap_or_else(|| format!("Display {}", i + 1)),
            x: m.position().x,
            y: m.position().y,
            width: m.size().width,
            height: m.size().height,
            is_primary: m.primary,
        })
        .collect()
}

fn mode_key(kind: &str) -> (&'static str, &'static str) {
    if kind == "dock" { ("nectar-dock-mode", "smart") } else { ("nectar-notch-mode", "fixed") }
}

#[tauri::command]
pub fn get_dock_mode_for_window() -> String {
    let (k, d) = mode_key("dock");
    crate::utils::setting_str(k).unwrap_or_else(|| d.into())
}

#[tauri::command]
pub fn get_notch_mode_for_window() -> String {
    let (k, d) = mode_key("notch");
    crate::utils::setting_str(k).unwrap_or_else(|| d.into())
}

#[tauri::command]
pub fn set_dock_mode_for_window(app: AppHandle, mode: String) -> Result<(), String> {
    save_setting(app, "nectar-dock-mode".into(), serde_json::Value::String(mode))
}

#[tauri::command]
pub fn set_notch_mode_for_window(app: AppHandle, mode: String) -> Result<(), String> {
    save_setting(app, "nectar-notch-mode".into(), serde_json::Value::String(mode))
}

// ---------------------------------------------------------------- overlay / splash

#[tauri::command]
pub fn hide_overlay(app: AppHandle) {
    if let Some(w) = app.get_webview_window("overlay") {
        let _ = w.hide();
    }
}

#[tauri::command]
pub fn set_splash_fullscreen(app: AppHandle, fullscreen: bool) {
    let already = OVERLAY_IN_SPLASH.swap(fullscreen, Ordering::Relaxed);
    let Some(win) = app.get_webview_window("overlay") else { return };
    if fullscreen {
        if already && win.is_visible().unwrap_or(false) {
            return;
        }
        if let Some(m) = crate::services::primary_monitor(&app) {
            let _ = win.set_position(*m.position());
            let _ = win.set_size(*m.size());
        }
        let _ = win.show();
    } else {
        SPLASH_FINISHED.store(true, Ordering::Relaxed);
        crate::services::sync_overlay(&app);
        // The overlay stays shown (click-through) for the volume and brightness HUDs.
        let _ = win.show();
        let _ = win.set_ignore_cursor_events(true);
    }
}

#[tauri::command]
pub fn sync_overlay_position(app: AppHandle) {
    crate::services::sync_overlay(&app);
}

#[tauri::command]
pub fn hide_native_osd() {}

// ---------------------------------------------------------------- on-demand windows

fn open_on_demand(app: &AppHandle, label: &str, url: &str, title: &str, w: f64, h: f64, min_w: f64, min_h: f64) {
    if let Some(win) = app.get_webview_window(label) {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
        return;
    }
    let app = app.clone();
    let (label, url, title) = (label.to_string(), url.to_string(), title.to_string());
    tauri::async_runtime::spawn(async move {
        let built = tauri::WebviewWindowBuilder::new(&app, &label, tauri::WebviewUrl::App(url.into()))
            .title(&title)
            .inner_size(w, h)
            .min_inner_size(min_w, min_h)
            .decorations(false)
            .transparent(true)
            .resizable(true)
            .maximizable(false)
            .closable(true)
            .focused(true)
            .skip_taskbar(false)
            .shadow(true)
            .visible(true)
            .center()
            .build();
        if let Ok(win) = built {
            let w2 = win.clone();
            win.on_window_event(move |e| {
                if let tauri::WindowEvent::CloseRequested { api, .. } = e {
                    api.prevent_close();
                    let _ = w2.hide();
                }
            });
        }
    });
}

#[tauri::command]
pub fn open_settings_window(app: AppHandle) {
    open_on_demand(&app, "settings", "settings.html", "Nectar Settings", 760.0, 620.0, 620.0, 480.0);
}

#[tauri::command]
pub fn open_calendar_window(app: AppHandle) {
    open_on_demand(&app, "calendar", "calendar.html", "Nectar Calendar", 880.0, 600.0, 700.0, 480.0);
}

#[tauri::command]
pub fn on_demand_window_ready(app: AppHandle, window: Window) {
    if let Some(win) = app.get_webview_window(window.label()) {
        let _ = win.show();
        let _ = win.set_focus();
    }
}

// ---------------------------------------------------------------- shell actions

fn open_desktop_menu() {
    // Super opens the Cinnamon/GNOME/KDE app menu.
    crate::x11::send_key_chord(&[crate::x11::keysym::SUPER_L]);
}

#[tauri::command]
pub fn show_desktop() {
    std::thread::spawn(crate::x11::toggle_showing_desktop);
}

#[tauri::command]
pub fn open_tray_at_cursor() {}

#[tauri::command]
pub fn open_tray_from_button(dx: f64, dy: f64) {
    let _ = (dx, dy);
}

#[tauri::command]
pub fn open_tray_window(app: AppHandle) {
    let _ = app;
}

fn open_first(candidates: &[(&str, &[&str])]) {
    for (cmd, args) in candidates {
        if crate::utils::which(cmd) && crate::utils::spawn_detached(cmd, args) {
            return;
        }
    }
}

#[tauri::command]
pub fn open_wifi_settings() {
    open_first(&[("cinnamon-settings", &["network"]), ("gnome-control-center", &["wifi"]), ("nm-connection-editor", &[])]);
}

#[tauri::command]
pub fn open_sound_settings() {
    open_first(&[("cinnamon-settings", &["sound"]), ("gnome-control-center", &["sound"]), ("pavucontrol", &[])]);
}

#[tauri::command]
pub fn open_notification_center() {
    open_first(&[("cinnamon-settings", &["notifications"]), ("gnome-control-center", &["notifications"])]);
}

#[tauri::command]
pub fn open_bluetooth_settings() {
    open_first(&[("blueman-manager", &[]), ("cinnamon-settings", &["bluetooth"]), ("gnome-control-center", &["bluetooth"])]);
}

#[tauri::command]
pub fn open_airplane_mode_settings() {
    open_wifi_settings();
}

#[tauri::command]
pub fn open_battery_saver_settings() {
    open_first(&[("cinnamon-settings", &["power"]), ("gnome-control-center", &["power"])]);
}

#[tauri::command]
pub fn open_ethernet_settings() {
    open_first(&[("cinnamon-settings", &["network"]), ("gnome-control-center", &["network"]), ("nm-connection-editor", &[])]);
}

// ---------------------------------------------------------------- media

#[tauri::command]
pub fn select_media_session(id: Option<String>) {
    if let Ok(mut s) = crate::media::SELECTED.lock() {
        *s = id.filter(|s| !s.is_empty());
    }
}

#[tauri::command]
pub fn media_play_pause() {
    std::thread::spawn(|| crate::media::control("play-pause"));
}

#[tauri::command]
pub fn media_next() {
    std::thread::spawn(|| crate::media::control("next"));
}

#[tauri::command]
pub fn media_previous() {
    std::thread::spawn(|| crate::media::control("previous"));
}

#[tauri::command]
pub fn media_seek(position_ms: f64) {
    std::thread::spawn(move || crate::media::seek(position_ms as i64));
}

#[tauri::command]
pub fn get_media_info() -> Option<MediaInfo> {
    crate::media::LAST.lock().ok()?.clone()
}

#[derive(serde::Serialize)]
pub struct AppMediaSession {
    pub app_id: String,
    pub title: String,
    pub artist: String,
    pub is_playing: bool,
}

#[tauri::command]
pub async fn get_app_media_sessions() -> Vec<AppMediaSession> {
    tauri::async_runtime::spawn_blocking(|| {
        crate::media::players()
            .into_iter()
            .map(|p| AppMediaSession { app_id: p.id, title: p.title, artist: p.artist, is_playing: p.status == "Playing" })
            .collect()
    })
    .await
    .unwrap_or_default()
}

#[tauri::command]
pub async fn app_media_control(app_id: String, action: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || crate::media::control_player(&app_id, &action))
        .await
        .map_err(|e| e.to_string())?
}

/// Brings the window of the app that's playing to the front.
#[tauri::command]
pub fn open_media_source_app() {
    std::thread::spawn(|| {
        let id = crate::media::ACTIVE.lock().map(|g| g.clone()).unwrap_or_default();
        let name = id.split('.').next().unwrap_or("").to_lowercase();
        if name.is_empty() {
            return;
        }
        for w in crate::x11::client_list() {
            let (inst, class) = crate::x11::wm_class(w);
            if inst.to_lowercase().contains(&name) || class.to_lowercase().contains(&name) {
                crate::x11::activate(w);
                return;
            }
        }
    });
}

// ---------------------------------------------------------------- running windows

fn app_info_for_window(win: u32) -> Option<(String, AppInfo)> {
    if !crate::x11::is_taskbar_window(win) {
        return None;
    }
    let pid = crate::x11::window_pid(win);
    if pid == Some(std::process::id()) {
        return None;
    }
    let title = crate::x11::window_title(win);
    let (instance, class) = crate::x11::wm_class(win);
    let gtk_id = crate::x11::gtk_application_id(win);
    let exe = pid.and_then(crate::system::process_exe);
    let entry = crate::desktop::match_window(&instance, &class, gtk_id.as_deref(), exe.as_deref());
    let title = if title.is_empty() { entry.as_ref().map(|e| e.name.clone()).unwrap_or_else(|| class.clone()) } else { title };
    let (path, executable, key) = match &entry {
        Some(e) => (e.path.clone(), Some(e.id.clone()), e.path.clone()),
        None => {
            let p = exe.clone().unwrap_or_else(|| class.clone());
            (p.clone(), Some(if class.is_empty() { instance.clone() } else { class.to_lowercase() }), format!("{p}:{class}"))
        }
    };
    Some((
        key,
        AppInfo {
            name: title,
            path,
            icon: None,
            is_running: true,
            hwnd: Some(win as isize),
            executable,
            all_hwnds: None,
            args: None,
        },
    ))
}

pub fn active_windows() -> Vec<AppInfo> {
    let mut grouped: Vec<(String, AppInfo)> = Vec::new();
    for win in crate::x11::client_list() {
        let Some((key, app)) = app_info_for_window(win) else { continue };
        let entry = (app.hwnd.unwrap_or(0), app.name.clone());
        match grouped.iter_mut().find(|(k, _)| *k == key) {
            Some((_, existing)) => existing.all_hwnds.get_or_insert_with(Vec::new).push(entry),
            None => {
                let mut a = app;
                a.all_hwnds = Some(vec![entry]);
                grouped.push((key, a));
            }
        }
    }
    let ts = focus_timestamps().lock().map(|g| g.clone()).unwrap_or_default();
    grouped
        .into_iter()
        .map(|(_, mut a)| {
            if let Some(h) = a.all_hwnds.as_mut() {
                h.sort_by_key(|(w, _)| std::cmp::Reverse(ts.get(w).copied().unwrap_or(0)));
                if let Some((w, t)) = h.first() {
                    a.hwnd = Some(*w);
                    // Single-window apps show the window title; multi-window ones the app name.
                    if h.len() == 1 {
                        a.name = t.clone();
                    }
                }
            }
            if a.all_hwnds.as_ref().is_some_and(|h| h.len() > 1) {
                if let Some(e) = crate::desktop::by_path(&a.path) {
                    a.name = e.name;
                }
            }
            a
        })
        .collect()
}

#[tauri::command]
pub async fn get_active_windows() -> Vec<AppInfo> {
    tauri::async_runtime::spawn_blocking(active_windows).await.unwrap_or_default()
}

static LAST_FOCUS_CLICK: std::sync::Mutex<Option<(isize, i64)>> = std::sync::Mutex::new(None);

/// Two clicks on the same window within 350 ms count as one.
fn is_repeat_click(hwnd: isize) -> bool {
    let now = crate::utils::get_now_ms();
    let Ok(mut g) = LAST_FOCUS_CLICK.lock() else { return false };
    let repeat = matches!(*g, Some((h, at)) if h == hwnd && now - at < 350);
    if !repeat {
        *g = Some((hwnd, now));
    }
    repeat
}

#[tauri::command]
pub async fn bring_window_forward(hwnd: isize) {
    std::thread::spawn(move || crate::x11::activate(hwnd as u32));
}

/// Dock click on a single window: restore if minimized, minimize if it is the window
/// in front, otherwise focus it. Only the exact window counts as "in front", never a
/// sibling window of the same app (Chrome runs all windows in one process, #154).
#[tauri::command]
pub async fn focus_window(hwnd: isize) {
    if is_repeat_click(hwnd) {
        return;
    }
    let _ = tauri::async_runtime::spawn_blocking(move || {
        let win = hwnd as u32;
        if crate::x11::is_minimized(win) {
            crate::x11::activate(win);
            crate::diagnostics::log(&format!("focus: restored {win:#x}"));
            return;
        }
        // If Nectar itself took focus from the click, the window that had it before counts.
        let active = crate::x11::active_window();
        let in_front = active == Some(win)
            || (active.is_some_and(|a| crate::x11::window_pid(a) == Some(std::process::id()))
                && crate::services::LAST_ACTIVE.load(Ordering::Relaxed) == win);
        if in_front {
            crate::x11::minimize(win);
        } else {
            crate::x11::activate(win);
        }
    })
    .await;
}

/// Multi-window item: windows are most recent first. If none is in front take the first,
/// otherwise go to the next one so repeated clicks cycle through them.
#[tauri::command]
pub async fn focus_app_windows(hwnds: Vec<isize>) {
    let Some(&first) = hwnds.first() else { return };
    if is_repeat_click(first) {
        return;
    }
    let _ = tauri::async_runtime::spawn_blocking(move || {
        let active = crate::x11::active_window().map(|w| w as isize);
        let mut ring = hwnds.clone();
        ring.sort_unstable();
        let target = match active.and_then(|a| ring.iter().position(|&h| h == a)) {
            Some(i) => ring[(i + 1) % ring.len()],
            None => first,
        };
        crate::x11::activate(target as u32);
    })
    .await;
}

#[tauri::command]
pub async fn close_window(hwnd: isize) {
    std::thread::spawn(move || crate::x11::close(hwnd as u32));
}

#[tauri::command]
pub async fn end_task(hwnd: isize, all_hwnds: Option<Vec<isize>>) {
    let mut wins = all_hwnds.unwrap_or_default();
    if !wins.contains(&hwnd) {
        wins.push(hwnd);
    }
    std::thread::spawn(move || {
        let mut pids: Vec<u32> = wins.iter().filter_map(|&w| crate::x11::window_pid(w as u32)).collect();
        pids.sort_unstable();
        pids.dedup();
        for pid in pids {
            let _ = crate::system::kill_process(pid);
        }
    });
}

#[tauri::command]
pub async fn focus_agent_window(window: u64) {
    std::thread::spawn(move || crate::x11::activate(window as u32));
}

#[tauri::command]
pub fn get_attention_windows() -> Vec<isize> {
    Vec::new()
}

#[tauri::command]
pub async fn capture_window_thumbnail(hwnd: isize, max_width: u32, max_height: u32) -> Result<Option<(String, i64)>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let img = crate::x11::capture_window(hwnd as u32, max_width.max(16), max_height.max(16))?;
        let data = crate::utils::rgba_to_png_data_url(img)?;
        let ts = focus_timestamps().lock().ok().and_then(|g| g.get(&hwnd).copied()).unwrap_or(0);
        Some((data, ts))
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_live_thumbnails(window: Window, targets: serde_json::Value, client_area_only: Option<bool>) -> Result<Vec<isize>, String> {
    let _ = (window, targets, client_area_only);
    Ok(Vec::new())
}

#[tauri::command]
pub fn set_live_thumbnails_opacity(opacity: u8) -> Result<(), String> {
    let _ = opacity;
    Ok(())
}

#[tauri::command]
pub fn clear_live_thumbnails() -> Result<(), String> {
    Ok(())
}

/// No DWM on Linux; returning false keeps the dock on static captured previews.
#[tauri::command]
pub fn is_dwm_composition_enabled() -> Result<bool, String> {
    Ok(false)
}

// ---------------------------------------------------------------- apps and icons

#[tauri::command]
pub async fn open_app(app: AppHandle, window: Window, app_name: String, args: Option<String>) {
    let _ = window;
    match app_name.as_str() {
        "start" | "search" => {
            std::thread::spawn(open_desktop_menu);
        }
        "taskview" => {
            // Cinnamon/Muffin: Ctrl+Alt+Down shows all windows of the workspace.
            std::thread::spawn(|| {
                use crate::x11::keysym::*;
                crate::x11::send_key_chord(&[CONTROL_L, ALT_L, DOWN]);
            });
        }
        "taskmgr" => {
            std::thread::spawn(|| open_first(&[("gnome-system-monitor", &[]), ("mate-system-monitor", &[]), ("xfce4-taskmanager", &[]), ("ksysguard", &[])]));
        }
        "nectar-settings" => open_settings_window(app),
        _ => {
            std::thread::spawn(move || {
                if !crate::desktop::launch(&app_name, args.as_deref()) {
                    crate::diagnostics::log(&format!("launch failed: {app_name}"));
                }
            });
        }
    }
}

#[tauri::command]
pub async fn launch_new_instance(app_path: String, app_name: Option<String>, args: Option<String>) {
    let _ = app_name;
    if app_path == "start" {
        return;
    }
    std::thread::spawn(move || {
        crate::desktop::launch(&app_path, args.as_deref());
    });
}

fn custom_icons_path(app: &AppHandle) -> Option<std::path::PathBuf> {
    app.path().app_config_dir().ok().map(|p| p.join("custom_icons.json"))
}

fn custom_icons(app: &AppHandle) -> HashMap<String, String> {
    custom_icons_path(app)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or_default()
}

fn icon_for(path: &str, name: Option<&str>, hwnd: Option<isize>) -> Option<String> {
    // A desktop entry's themed icon looks best; the window's own icon is the fallback.
    if let Some(entry) = crate::desktop::by_path(path) {
        if let Some(icon) = entry.icon.as_deref().and_then(|i| crate::desktop::icon_data_url(i, 128)) {
            return Some(icon);
        }
    }
    if let Some(h) = hwnd.filter(|&h| h != 0) {
        if let Some(img) = crate::x11::window_icon(h as u32, 128) {
            return crate::utils::rgba_to_png_data_url(img);
        }
    }
    let p = std::path::Path::new(path);
    if p.is_file() && matches!(p.extension().and_then(|e| e.to_str()), Some("png" | "svg" | "jpg" | "jpeg" | "ico")) {
        return crate::utils::image_file_to_data_url(p, 128);
    }
    // Plain program: look for an entry running that binary, or a themed icon by name.
    let base = p.file_name().map(|f| f.to_string_lossy().to_lowercase()).unwrap_or_else(|| path.to_lowercase());
    if let Some(entry) = crate::desktop::entries().into_iter().find(|e| e.exec_binary().as_deref() == Some(base.as_str())) {
        if let Some(icon) = entry.icon.as_deref().and_then(|i| crate::desktop::icon_data_url(i, 128)) {
            return Some(icon);
        }
    }
    if p.is_dir() {
        return crate::desktop::icon_data_url("folder", 128);
    }
    let _ = name;
    crate::desktop::icon_data_url(&base, 128).or_else(|| crate::desktop::icon_data_url("application-x-executable", 128))
}

#[tauri::command]
pub async fn get_app_icon(app: AppHandle, path: String, name: Option<String>, hwnd: Option<isize>) -> Result<Option<String>, String> {
    if path == "nectar-settings" {
        return Ok(crate::desktop::icon_data_url("preferences-system", 128));
    }
    let custom = custom_icons(&app);
    if let Some(c) = custom.get(&path) {
        return Ok(Some(c.clone()));
    }
    let key = format!("{path}|{}", hwnd.filter(|_| crate::desktop::by_path(&path).is_none()).unwrap_or(0));
    if let Some(c) = icon_cache().lock().ok().and_then(|g| g.get(&key).cloned()) {
        return Ok(Some(c));
    }
    let result = tauri::async_runtime::spawn_blocking(move || icon_for(&path, name.as_deref(), hwnd))
        .await
        .map_err(|e| e.to_string())?;
    if let Some(icon) = &result {
        if let Ok(mut g) = icon_cache().lock() {
            g.insert(key, icon.clone());
        }
    }
    Ok(result)
}

#[tauri::command]
pub async fn clear_icon_cache(app: AppHandle) -> Result<(), String> {
    if let Ok(mut g) = icon_cache().lock() {
        g.clear();
    }
    crate::desktop::invalidate();
    let _ = app.emit("icon-cache-cleared", ());
    Ok(())
}

#[tauri::command]
pub async fn set_custom_icon(app: AppHandle, cache_key: String, icon_data: String) -> Result<String, String> {
    let data = if icon_data.starts_with("data:") {
        icon_data
    } else {
        // A file path from the picker.
        crate::utils::image_file_to_data_url(std::path::Path::new(&icon_data), 256).ok_or("Couldn't read that image")?
    };
    let mut map = custom_icons(&app);
    map.insert(cache_key, data.clone());
    let path = custom_icons_path(&app).ok_or("No config folder")?;
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(path, serde_json::to_string(&map).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    Ok(data)
}

#[tauri::command]
pub async fn remove_custom_icon(app: AppHandle, path: String, name: Option<String>) -> Result<(), String> {
    let mut map = custom_icons(&app);
    map.remove(&path);
    if let Some(n) = name {
        map.remove(&format!("{path}:{}", n.to_lowercase()));
    }
    let file = custom_icons_path(&app).ok_or("No config folder")?;
    std::fs::write(file, serde_json::to_string(&map).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    if let Ok(mut g) = icon_cache().lock() {
        g.retain(|k, _| !k.starts_with(&format!("{path}|")));
    }
    Ok(())
}

#[tauri::command]
pub async fn get_custom_icons(app: AppHandle) -> Result<HashMap<String, String>, String> {
    Ok(custom_icons(&app))
}

#[tauri::command]
pub async fn get_installed_apps() -> Vec<AppInfo> {
    tauri::async_runtime::spawn_blocking(|| {
        crate::desktop::entries()
            .into_iter()
            .filter(|e| !e.no_display)
            .map(|e| AppInfo {
                name: e.name.clone(),
                path: e.path.clone(),
                icon: None,
                is_running: false,
                hwnd: None,
                executable: Some(e.id.clone()),
                all_hwnds: None,
                args: None,
            })
            .collect()
    })
    .await
    .unwrap_or_default()
}

#[tauri::command]
pub fn save_pinned_apps(app: AppHandle, apps: Vec<AppInfo>) -> Result<(), String> {
    let path = app.path().app_config_dir().map_err(|e| e.to_string())?.join("pinned_apps.json");
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(path, serde_json::to_string(&apps).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

fn pin(e: crate::desktop::DesktopEntry) -> AppInfo {
    AppInfo { name: e.name, path: e.path, icon: None, is_running: false, hwnd: None, executable: Some(e.id), all_hwnds: None, args: None }
}

/// Fresh install: file manager, browser, terminal and text editor that exist here, plus Settings.
fn default_pins() -> Vec<AppInfo> {
    let mut pins = Vec::new();
    let file_manager = crate::desktop::default_for("inode/directory")
        .or_else(|| crate::desktop::first_existing(&["nemo", "org.gnome.Nautilus", "thunar", "org.kde.dolphin", "caja", "pcmanfm"]));
    let browser = crate::desktop::default_for("x-scheme-handler/https")
        .or_else(|| crate::desktop::first_existing(&["firefox", "google-chrome", "chromium", "brave-browser"]));
    let terminal = crate::desktop::first_existing(&["org.gnome.Terminal", "gnome-terminal", "xfce4-terminal", "org.kde.konsole", "mate-terminal", "xterm"]);
    let editor = crate::desktop::default_for("text/plain")
        .or_else(|| crate::desktop::first_existing(&["org.x.editor", "xed", "org.gnome.TextEditor", "gedit", "mousepad", "org.kde.kate"]));
    for e in [file_manager, browser, terminal, editor].into_iter().flatten() {
        if !pins.iter().any(|p: &AppInfo| p.path == e.path) {
            pins.push(pin(e));
        }
    }
    pins.push(AppInfo { name: "Settings".into(), path: "nectar-settings".into(), icon: None, is_running: false, hwnd: None, executable: None, all_hwnds: None, args: None });
    pins
}

#[tauri::command]
pub async fn load_pinned_apps(app: AppHandle) -> Vec<AppInfo> {
    let path = app.path().app_config_dir().unwrap_or_default().join("pinned_apps.json");
    if let Ok(content) = std::fs::read_to_string(&path) {
        if let Ok(apps) = serde_json::from_str(&content) {
            return apps;
        }
    }
    tauri::async_runtime::spawn_blocking(default_pins).await.unwrap_or_default()
}

#[tauri::command]
pub fn get_drives() -> Vec<crate::system::DockDrive> {
    crate::system::drives()
}

#[tauri::command]
pub fn open_dock_extra(target: String) -> Result<(), String> {
    let target = match target.as_str() {
        "downloads" => crate::utils::run("xdg-user-dir", &["DOWNLOAD"]).unwrap_or_else(|| format!("{}/Downloads", std::env::var("HOME").unwrap_or_default())),
        "documents" => crate::utils::run("xdg-user-dir", &["DOCUMENTS"]).unwrap_or_default(),
        "recycle" | "trash" => "trash:///".into(),
        "home" => std::env::var("HOME").unwrap_or_default(),
        other => other.to_string(),
    };
    if crate::utils::spawn_detached("xdg-open", &[&target]) { Ok(()) } else { Err("xdg-open failed".into()) }
}

#[tauri::command]
pub async fn pick_program(app: AppHandle) -> Result<Option<(String, String)>, String> {
    let apps_dir = std::path::PathBuf::from("/usr/share/applications");
    let mut dialog = app.dialog().file().set_title("Choose a program").set_directory(apps_dir);
    if let Some(dock) = app.get_webview_window("dock") {
        dialog = dialog.set_parent(&dock);
    }
    let Some(path) = dialog.blocking_pick_file() else { return Ok(None) };
    let path = path.into_path().map_err(|e| e.to_string())?;
    let path_str = path.to_string_lossy().to_string();
    if let Some(entry) = crate::desktop::by_path(&path_str) {
        return Ok(Some((entry.name, entry.path)));
    }
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("App");
    let mut chars = stem.chars();
    let name = match chars.next() {
        Some(f) => f.to_uppercase().chain(chars).collect(),
        None => "App".to_string(),
    };
    Ok(Some((name, path_str)))
}

// ---------------------------------------------------------------- agent sounds

const AGENT_SOUND_STATES: [&str; 3] = ["needs_input", "done", "error"];

fn agent_sound_files(app: &AppHandle, state: &str) -> Vec<std::path::PathBuf> {
    let Ok(dir) = app.path().app_data_dir().map(|d| d.join("sounds")) else { return Vec::new() };
    std::fs::read_dir(dir)
        .map(|it| it.flatten().map(|e| e.path()).filter(|p| p.file_stem().and_then(|s| s.to_str()) == Some(state)).collect())
        .unwrap_or_default()
}

#[tauri::command]
pub async fn pick_agent_sound(app: AppHandle, state: String) -> Result<bool, String> {
    if !AGENT_SOUND_STATES.contains(&state.as_str()) {
        return Err("Unknown state".into());
    }
    let Some(path) = app.dialog().file().set_title("Choose a sound").add_filter("Audio", &["mp3", "wav", "ogg", "m4a"]).blocking_pick_file() else {
        return Ok(false);
    };
    let path = path.into_path().map_err(|e| e.to_string())?;
    if std::fs::metadata(&path).map_err(|e| e.to_string())?.len() > 3 * 1024 * 1024 {
        return Err("Sound is too large (maximum 3MB)".into());
    }
    let ext = path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).unwrap_or_default();
    if !["mp3", "wav", "ogg", "m4a"].contains(&ext.as_str()) {
        return Err("Unsupported file type".into());
    }
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?.join("sounds");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    for old in agent_sound_files(&app, &state) {
        let _ = std::fs::remove_file(old);
    }
    std::fs::copy(&path, dir.join(format!("{state}.{ext}"))).map_err(|e| e.to_string())?;
    let _ = app.emit("agent-sound-changed", ());
    Ok(true)
}

#[tauri::command]
pub async fn reset_agent_sound(app: AppHandle, state: String) {
    for old in agent_sound_files(&app, &state) {
        let _ = std::fs::remove_file(old);
    }
    let _ = app.emit("agent-sound-changed", ());
}

#[tauri::command]
pub async fn agent_sound_data(app: AppHandle, state: String) -> Option<String> {
    use base64::Engine;
    let path = agent_sound_files(&app, &state).into_iter().next()?;
    let mime = match path.extension()?.to_str()? {
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "ogg" => "audio/ogg",
        "m4a" => "audio/mp4",
        _ => return None,
    };
    let bytes = std::fs::read(path).ok()?;
    Some(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
}

#[tauri::command]
pub async fn agent_sound_custom(app: AppHandle) -> Vec<String> {
    AGENT_SOUND_STATES.iter().filter(|s| !agent_sound_files(&app, s).is_empty()).map(|s| s.to_string()).collect()
}

#[tauri::command]
pub fn agent_answer(id: u64, allow: bool, choices: Option<Vec<usize>>) {
    let _ = (id, allow, choices);
}

// ---------------------------------------------------------------- shelf (drag & drop tray)

#[tauri::command]
pub fn setup_shelf_drop() {}

#[tauri::command]
pub async fn shelf_thumbnail(path: String) -> Option<String> {
    tauri::async_runtime::spawn_blocking(move || {
        let p = std::path::Path::new(&path);
        let ext = p.extension()?.to_str()?.to_lowercase();
        if matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "svg") {
            return crate::utils::image_file_to_data_url(p, 256);
        }
        None
    })
    .await
    .ok()
    .flatten()
}

#[tauri::command]
pub async fn shelf_text_preview(path: String) -> Option<String> {
    let bytes = std::fs::read(&path).ok()?;
    let text = String::from_utf8(bytes.into_iter().take(2000).collect()).ok()?;
    Some(text.chars().take(400).collect())
}

#[tauri::command]
pub async fn shelf_open(path: String) -> Result<(), String> {
    if crate::utils::spawn_detached("xdg-open", &[&path]) { Ok(()) } else { Err("xdg-open failed".into()) }
}

#[tauri::command]
pub async fn shelf_prune(paths: Vec<String>) -> Vec<String> {
    paths.into_iter().filter(|p| std::path::Path::new(p).exists()).collect()
}

#[tauri::command]
pub fn shelf_drag_icon() -> Option<String> {
    crate::desktop::resolve_icon("text-x-generic", 64).map(|p| p.to_string_lossy().to_string())
}

#[tauri::command]
pub fn shelf_text_file(text: String) -> Option<String> {
    let dir = std::env::temp_dir().join("nectar-shelf");
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join(format!("text-{}.txt", crate::utils::get_now_ms()));
    std::fs::write(&path, text).ok()?;
    Some(path.to_string_lossy().to_string())
}

// ---------------------------------------------------------------- audio

#[tauri::command]
pub fn get_volume() -> f32 {
    crate::services::CURRENT_VOLUME.lock().ok().and_then(|g| *g).map(|(v, _)| v).unwrap_or(0.5)
}

#[tauri::command]
pub async fn get_volume_state() -> Result<VolumeChangeEvent, String> {
    let (volume, is_muted) = tauri::async_runtime::spawn_blocking(crate::system::volume_state)
        .await
        .map_err(|e| e.to_string())?
        .ok_or("No audio output")?;
    Ok(VolumeChangeEvent { volume, is_muted })
}

#[tauri::command]
pub fn set_volume(volume: f32) {
    std::thread::spawn(move || crate::system::set_volume(volume));
}

#[tauri::command]
pub async fn get_audio_sessions() -> Result<Vec<AudioSessionInfo>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let mut out: Vec<AudioSessionInfo> = Vec::new();
        for s in crate::system::sink_inputs() {
            if out.iter().any(|o| o.pid == s.pid && s.pid != 0) {
                continue;
            }
            let process_path = if s.pid != 0 { crate::system::process_exe(s.pid) } else { None };
            out.push(AudioSessionInfo { pid: s.pid, name: s.name, process_path: process_path.or(s.binary), volume: s.volume, is_muted: s.muted });
        }
        out
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_app_volume(pid: u32, volume: f32) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || crate::system::set_sink_input_volume(pid, volume)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn set_app_mute(pid: u32, muted: bool) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || crate::system::set_sink_input_mute(pid, muted)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn set_volume_mixer_rect(x: f64, y: f64, width: f64, height: f64) {
    if let Ok(mut r) = VOLUME_MIXER_RECT.lock() {
        *r = Some((x, y, width, height));
    }
}

#[tauri::command]
pub fn clear_volume_mixer_rect() {
    if let Ok(mut r) = VOLUME_MIXER_RECT.lock() {
        *r = None;
    }
}

#[tauri::command]
pub async fn get_audio_devices() -> Result<Vec<crate::system::AudioDevice>, String> {
    tauri::async_runtime::spawn_blocking(crate::system::audio_devices).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_default_audio_device(id: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || crate::system::set_default_audio_device(&id)).await.map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------- brightness / power

#[tauri::command]
pub fn get_brightness() -> u32 {
    crate::system::get_brightness().unwrap_or(100)
}

#[tauri::command]
pub fn set_brightness(app: AppHandle, brightness: u32) {
    std::thread::spawn(move || {
        if crate::system::set_brightness(brightness) {
            let _ = app.emit("brightness-change", crate::types::BrightnessChangeEvent { brightness });
        }
    });
}

#[tauri::command]
pub async fn get_battery_saver_state() -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(crate::system::battery_saver).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn has_battery() -> Result<bool, String> {
    Ok(crate::system::has_battery())
}

// ---------------------------------------------------------------- network

#[tauri::command]
pub async fn get_wifi_status() -> Result<WifiStatus, String> {
    tauri::async_runtime::spawn_blocking(|| WifiStatus { enabled: crate::system::wifi_enabled(), connected: crate::system::wifi_connected() })
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_wifi_state(enabled: bool) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || crate::system::set_wifi(enabled)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn get_bluetooth_state() -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(crate::system::bluetooth_powered).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_bluetooth_state(enabled: bool) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || crate::system::set_bluetooth(enabled)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn wifi_scan() -> Result<Vec<crate::system::WifiNetwork>, String> {
    tauri::async_runtime::spawn_blocking(crate::system::wifi_scan).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn wifi_connect(ssid: String, password: Option<String>) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || crate::system::wifi_connect(&ssid, password.as_deref())).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn wifi_disconnect() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(crate::system::wifi_disconnect).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn wifi_forget(ssid: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || crate::system::wifi_forget(&ssid)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn get_lan_status() -> crate::system::LanStatus {
    tauri::async_runtime::spawn_blocking(crate::system::lan_status)
        .await
        .unwrap_or_else(|_| crate::system::lan_status())
}

#[tauri::command]
pub async fn bluetooth_devices() -> Result<Vec<crate::system::BluetoothDeviceInfo>, String> {
    tauri::async_runtime::spawn_blocking(crate::system::bluetooth_devices).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn bluetooth_unpair(id: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || crate::system::bluetooth_unpair(&id)).await.map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------- metrics

#[derive(serde::Serialize)]
pub struct SystemMetrics {
    cpu: u32,
    ram: f32,
    disk_free_gb: u64,
    net_up: u64,
    net_down: u64,
}

#[tauri::command]
pub fn get_system_metrics() -> SystemMetrics {
    let (net_up, net_down) = crate::system::network_speed();
    SystemMetrics { cpu: crate::system::cpu_usage(), ram: crate::system::ram_usage(), disk_free_gb: crate::system::disk_free_gb(), net_up, net_down }
}

#[tauri::command]
pub fn get_cpu_usage() -> Result<u32, String> {
    Ok(crate::system::cpu_usage())
}

#[tauri::command]
pub fn get_ram_usage() -> Result<f32, String> {
    Ok(crate::system::ram_usage())
}

#[tauri::command]
pub fn get_disk_space() -> Result<u64, String> {
    Ok(crate::system::disk_free_gb())
}

#[tauri::command]
pub fn get_network_speed() -> Result<(u64, u64), String> {
    Ok(crate::system::network_speed())
}

#[tauri::command]
pub async fn get_ping_ms(app: AppHandle) -> Option<u32> {
    let target = get_setting_str(&app, "nectar-ping-host").unwrap_or_else(|| "1.1.1.1:443".to_string());
    tauri::async_runtime::spawn_blocking(move || {
        use std::net::{TcpStream, ToSocketAddrs};
        let addr = target.to_socket_addrs().ok()?.next()?;
        let start = std::time::Instant::now();
        TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(1500)).ok()?;
        Some(start.elapsed().as_millis() as u32)
    })
    .await
    .ok()
    .flatten()
}

#[tauri::command]
pub fn get_running_processes() -> Result<Vec<crate::system::ProcessInfo>, String> {
    Ok(crate::system::processes())
}

#[tauri::command]
pub fn kill_process(pid: u32, _name: Option<String>) -> Result<(), String> {
    crate::system::kill_process(pid)
}

// ---------------------------------------------------------------- accent / wallpaper

#[tauri::command]
pub fn get_system_accent_color() -> Result<String, String> {
    crate::system::accent_color().ok_or_else(|| "No accent color".into())
}

#[tauri::command]
pub async fn sync_windows_accent(hex: String) -> Result<(), String> {
    let _ = hex;
    Ok(())
}

#[tauri::command]
pub async fn restore_windows_accent() -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub fn windows_accent_backup_exists() -> bool {
    false
}

#[tauri::command]
pub async fn get_wallpaper_backdrop(blur: u32) -> Result<Option<serde_json::Value>, String> {
    let _ = blur;
    Ok(None)
}

#[tauri::command]
pub async fn is_backdrop_covered(rect: serde_json::Value) -> bool {
    let _ = rect;
    false
}

// ---------------------------------------------------------------- settings

pub fn write_settings(app: &AppHandle, settings: &HashMap<String, serde_json::Value>) -> Result<(), String> {
    let path = crate::utils::settings_path(app).ok_or("No config folder")?;
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    crate::services::SELF_WRITE_MS.store(crate::utils::get_now_ms(), Ordering::Relaxed);
    std::fs::write(path, serde_json::to_string(settings).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

const PLACEMENT_KEYS: [&str; 6] = [
    "nectar-scale",
    "nectar-dock-monitor-mode",
    "nectar-dock-monitor-id",
    "nectar-notch-monitor-mode",
    "nectar-notch-monitor-id",
    "nectar-dock-mode",
];

#[tauri::command]
pub fn save_setting(app: AppHandle, key: String, value: serde_json::Value) -> Result<(), String> {
    let mut settings = crate::utils::read_settings_file(&app);
    settings.insert(key.clone(), value.clone());
    write_settings(&app, &settings)?;
    crate::utils::replace_settings_cache(settings);
    let _ = app.emit("settings-changed", serde_json::json!({ "key": &key, "value": &value }));

    if key == "nectar-language" {
        crate::i18n::refresh_tray_labels(&app);
    }
    if key == "nectar-linux-hide-panel" && value.as_str() == Some("false") {
        crate::panel::set_visible(true);
    }
    if PLACEMENT_KEYS.contains(&key.as_str()) {
        let app2 = app.clone();
        tauri::async_runtime::spawn(async move {
            crate::services::position_notch(&app2);
            if dock_enabled(&app2) {
                let mode = get_setting_str(&app2, "nectar-dock-mode").unwrap_or_else(|| "smart".into());
                crate::services::position_dock(&app2, &mode);
            }
            crate::services::sync_overlay(&app2);
        });
    }
    Ok(())
}

#[tauri::command]
pub fn load_settings(app: AppHandle) -> Result<HashMap<String, serde_json::Value>, String> {
    Ok(crate::utils::read_settings_file(&app))
}

#[tauri::command]
pub fn reset_settings(app: AppHandle) -> Result<(), String> {
    write_settings(&app, &HashMap::new())?;
    crate::utils::replace_settings_cache(HashMap::new());
    Ok(())
}

#[tauri::command]
pub fn export_settings(app: AppHandle) -> Result<String, String> {
    let path = crate::utils::settings_path(&app).ok_or("No config folder")?;
    let content = std::fs::read_to_string(path).map_err(|_| "No settings file found".to_string())?;
    let _: HashMap<String, serde_json::Value> = serde_json::from_str(&content).map_err(|e| format!("Invalid settings file: {e}"))?;
    Ok(content)
}

#[tauri::command]
pub async fn export_settings_dialog(app: AppHandle) -> Result<bool, String> {
    let json = export_settings(app.clone())?;
    let Some(path) = app.dialog().file().set_title("Export Nectar Settings").set_file_name("nectar-settings.json").add_filter("JSON", &["json"]).blocking_save_file() else {
        return Ok(false);
    };
    std::fs::write(path.into_path().map_err(|e| e.to_string())?, json).map_err(|e| e.to_string())?;
    Ok(true)
}

#[tauri::command]
pub async fn import_settings_dialog(app: AppHandle) -> Result<bool, String> {
    let Some(path) = app.dialog().file().set_title("Import Nectar Settings").add_filter("JSON", &["json"]).blocking_pick_file() else {
        return Ok(false);
    };
    let path = path.into_path().map_err(|e| e.to_string())?;
    if std::fs::metadata(&path).map_err(|e| e.to_string())?.len() > 2 * 1024 * 1024 {
        return Err("Settings file is too large (maximum 2MB allowed)".into());
    }
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    import_settings(app, content)?;
    Ok(true)
}

#[tauri::command]
pub fn import_settings(app: AppHandle, settings: String) -> Result<(), String> {
    let imported: HashMap<String, serde_json::Value> = serde_json::from_str(&settings).map_err(|e| format!("Invalid JSON: {e}"))?;
    write_settings(&app, &imported)?;
    crate::utils::replace_settings_cache(imported.clone());
    for (k, v) in &imported {
        let _ = app.emit("settings-changed", serde_json::json!({ "key": k, "value": v }));
    }
    Ok(())
}

// ---------------------------------------------------------------- app lifecycle

pub fn restore_and_exit(app: &AppHandle) {
    SHUTTING_DOWN.store(true, Ordering::Relaxed);
    if let Some(xid) = crate::x11::find_own_window("Nectar Dock") {
        crate::x11::clear_strut(xid);
    }
    crate::panel::set_visible(true);
    app.exit(0);
}

#[tauri::command]
pub async fn quit_nectar(handle: AppHandle) {
    restore_and_exit(&handle);
}

#[tauri::command]
pub async fn restart_nectar(handle: AppHandle) {
    SHUTTING_DOWN.store(true, Ordering::Relaxed);
    std::env::set_var("NECTAR_RESTARTING", "1");
    handle.restart();
}

#[tauri::command]
pub async fn uninstall_nectar(handle: AppHandle) -> Result<(), String> {
    let _ = handle;
    Err("On Linux, remove Nectar with your package manager (for example: sudo apt remove nectar).".into())
}

#[tauri::command]
pub async fn get_diagnostics(app: AppHandle) -> String {
    let info = |k: &str| std::env::var(k).unwrap_or_default();
    format!(
        "Nectar {} (Linux)\nDesktop: {} / session {}\nX11: {}\nMonitors: {:?}\nWindows tracked: {}\n\n--- log tail ---\n{}",
        app.package_info().version,
        info("XDG_CURRENT_DESKTOP"),
        info("XDG_SESSION_TYPE"),
        crate::x11::available(),
        get_monitors(app.clone()).iter().map(|m| format!("{} {}x{}+{}+{}", m.label, m.width, m.height, m.x, m.y)).collect::<Vec<_>>(),
        crate::x11::client_list().len(),
        crate::diagnostics::read_tail(6000)
    )
}

#[tauri::command]
pub fn open_log_folder() -> Result<(), String> {
    let dir = crate::diagnostics::log_dir().ok_or("No log folder yet")?;
    if crate::utils::spawn_detached("xdg-open", &[&dir.to_string_lossy()]) { Ok(()) } else { Err("xdg-open failed".into()) }
}

#[tauri::command]
pub fn set_hitbox_logging(enabled: bool) {
    let _ = enabled;
}

#[tauri::command]
pub fn get_hitbox_logging() -> bool {
    false
}

#[tauri::command]
pub fn get_autostart(app: AppHandle) -> bool {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch().is_enabled().unwrap_or(false)
}

#[tauri::command]
pub fn set_autostart(app: AppHandle, enabled: bool) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let l = app.autolaunch();
    if enabled { l.enable() } else { l.disable() }.map_err(|e| e.to_string())
}

// ---------------------------------------------------------------- updates

#[derive(serde::Serialize, Clone, Default)]
pub struct UpdateCheckResult {
    pub available: bool,
}

/// Updates come from the package manager on Linux, the Windows updater doesn't apply.
#[tauri::command]
pub async fn check_for_updates(force: bool) -> Result<UpdateCheckResult, String> {
    let _ = force;
    Ok(UpdateCheckResult::default())
}

#[tauri::command]
pub async fn install_update() -> Result<(), String> {
    Err("Update Nectar with your package manager on Linux.".into())
}

#[tauri::command]
pub fn get_update_state() -> UpdateCheckResult {
    UpdateCheckResult::default()
}
