use tauri::{AppHandle, Emitter, Manager, Window};
use windows::core::Interface;
use windows::Win32::Foundation::{HWND, LPARAM};
use windows::Win32::UI::WindowsAndMessaging::EnumWindows;
use std::sync::atomic::Ordering;

use crate::types::{IntRect, AppInfo, BrightnessChangeEvent, WifiStatus, AudioSessionInfo, VolumeChangeEvent};
use crate::state::*;
use crate::utils::*;
use crate::services::{register_appbar, register_dock_appbar, sync_overlays, unregister_appbar_native, enum_windows_proc};
use std::collections::HashMap;

#[tauri::command]
pub async fn set_menu_open(open: bool, rect: Option<IntRect>) {
    MENU_IS_OPEN.store(open, Ordering::Relaxed);
    if let Ok(mut r) = MENU_RECT.lock() {
        *r = rect;
    }
}

#[tauri::command]
pub async fn set_dock_hovered(window: Window, hovered: bool) {
    crate::state::set_flag(crate::state::dock_hovered(), window.label(), hovered);
}

#[tauri::command]
pub async fn set_notch_hovered(window: Window, hovered: bool) {
    crate::state::set_flag(crate::state::notch_hovered(), window.label(), hovered);
}

#[tauri::command]
pub async fn set_notch_hidden(window: Window, hidden: bool) {
    crate::state::set_flag(crate::state::notch_hidden(), window.label(), hidden);
}

#[tauri::command]
pub async fn set_dock_dragging(dragging: bool) {
    crate::state::DOCK_IS_DRAGGING.store(dragging, Ordering::Relaxed);
}

#[tauri::command]
pub async fn update_dock_rect(window: Window, rect: IntRect) {
    if let Ok(mut m) = crate::state::dock_rects().lock() {
        m.insert(window.label().to_string(), rect);
    }
}

#[tauri::command]
pub async fn update_notch_rect(window: Window, rect: IntRect) {
    if let Ok(mut m) = crate::state::notch_rects().lock() {
        m.insert(window.label().to_string(), rect);
    }
}

#[tauri::command]
pub fn set_window_height(window: Window, height: f64) {
    if let Ok(scale_factor) = window.scale_factor() {
        if let Ok(physical_size) = window.inner_size() {
            let logical_width = physical_size.width as f64 / scale_factor;
            let _ = window.set_size(tauri::LogicalSize::new(logical_width, height));
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
    let _ = window.set_ignore_cursor_events(ignore);
}

#[tauri::command]
pub async fn init_dock(app: AppHandle, mode: String) {
    // Backend guard: bail if dock is disabled in settings.
    // The frontend already checks this, but settings.json may have a stale
    // value if the write didn't complete before restart. Reading here too
    // makes the dock reliably stay hidden regardless of frontend timing.
    let enabled = get_setting_str(&app, "nectar-dock-enabled")
        .unwrap_or_else(|| "true".to_string());
    if enabled != "true" {
        if let Some(dock_win) = app.get_webview_window("dock") {
            let _ = dock_win.hide();
            crate::state::set_flag(crate::state::dock_appbar_registered(), "dock", false);
        }
        return;
    }

    if let Some(dock_win) = app.get_webview_window("dock") {
        init_dock_window(&app, dock_win, &mode).await;
    }
}

pub async fn init_dock_window(app: &AppHandle, dock_win: tauri::WebviewWindow, mode: &str) {
    let label = dock_win.label().to_string();

    // 1. Always show first — idempotent, required before any positioning
    if !dock_win.is_visible().unwrap_or(false) {
        let _ = dock_win.set_position(tauri::PhysicalPosition::new(-32000, -32000));
    }
    let _ = dock_win.show();
    if let Ok(hwnd) = dock_win.hwnd() { re_assert_topmost(hwnd); }

    // 2. Register as appbar (fixed) or manually position (auto-hide)
    if mode == "fixed" {
        // register_dock_appbar calls show() internally too, and handles retries
        register_dock_appbar(dock_win.clone());
    } else {
        let dock_clone = dock_win.clone();
        tauri::async_runtime::spawn(async move {
            for attempt in 0..20 {
                // Wait for monitor and window dimensions to be available.
                // Never use a hardcoded fallback — wrong values produce off-screen placement.
                // Extract HWND as isize before any await (raw pointer is not Send).
                let hwnd_val = dock_clone.hwnd().map(|h| h.0 as isize).unwrap_or(0);
                let ph = dock_clone.outer_size().map(|s| s.height as i32).unwrap_or(0);
                let monitor_info = crate::monitors::monitor_for_window_label(dock_clone.app_handle(), crate::types::WindowKind::Dock, dock_clone.label()).map(|m| {
                    let s = m.size();
                    let p = m.position();
                    (tauri::PhysicalSize::new(s.width, s.height), tauri::PhysicalPosition::new(p.x, p.y))
                });

                if hwnd_val != 0 {
                    if let Some((m_size, m_pos)) = monitor_info {
                        if ph <= 10 {
                            // outer_size() not ready yet — retry next tick
                            if attempt < 19 {
                                tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                            }
                            continue;
                        }
                        let final_y = m_pos.y + m_size.height as i32 - ph;
                        unsafe {
                            use windows::Win32::UI::WindowsAndMessaging::{
                                SetWindowPos, SWP_NOZORDER, SWP_NOACTIVATE, SWP_FRAMECHANGED
                            };
                            use windows::Win32::Foundation::HWND;
                            let _ = SetWindowPos(
                                HWND(hwnd_val as *mut _), None,
                                m_pos.x, final_y,
                                m_size.width as i32, ph,
                                SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED
                            );
                        }
                        // Re-assert topmost after repositioning
                        if let Ok(hwnd) = dock_clone.hwnd() { re_assert_topmost(hwnd); }
                        // Ensure visible after positioning
                        let _ = dock_clone.show();
                        break;
                    }
                }
                if attempt < 19 {
                    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                }
            }
        });
    }

    // 3. Hide taskbar after showing dock (not before, so user always has something)
    set_taskbar_visibility(false, false);
    NATIVE_TASKBAR_HIDDEN.store(true, Ordering::Relaxed);
    crate::tray_icons::schedule_taskbar_layout_refresh();

    // 4. Reset overlap state so the overlap thread re-syncs cleanly
    crate::state::set_overlap(crate::state::dock_overlap(), &label, 0);
    let _ = app.emit_to(label.as_str(), "dock-overlap", false);
    // Sync the maximized state so the adaptive dock expands right away.
    if label == "dock" {
        let _ = app.emit_to(label.as_str(), "dock-maximized", CURRENT_FOREGROUND_MAXIMIZED.load(Ordering::Relaxed));
    }
}

#[tauri::command]
pub async fn toggle_dock(app: AppHandle, enable: bool) {
    if let Some(dock_win) = app.get_webview_window("dock") {
        if enable {
            // Saved dock mode, smart on a fresh install.
            let saved_mode = crate::utils::get_setting_str(&app, "nectar-dock-mode")
                .unwrap_or_else(|| "smart".to_string());
            init_dock(app.clone(), saved_mode).await;
        } else {
            let _ = dock_win.hide();
            if let Ok(hwnd) = dock_win.hwnd() {
                let hwnd_val = hwnd.0 as isize;
                tauri::async_runtime::spawn_blocking(move || {
                    unregister_appbar_native(HWND(hwnd_val as *mut _));
                });
            }
            crate::state::set_flag(crate::state::dock_appbar_registered(), "dock", false);
            set_taskbar_visibility(true, true);
            NATIVE_TASKBAR_HIDDEN.store(false, Ordering::Relaxed);

            // Re-sync other appbars
            if let Some(main_win) = app.get_webview_window("main") {
                if crate::state::get_flag(crate::state::main_appbar_registered(), "main") {
                    register_appbar(main_win);
                }
            }
        }
    }
}

#[tauri::command]
pub async fn sync_appbar(app: AppHandle) {
    let dock_enabled = get_setting_str(&app, "nectar-dock-enabled")
        .unwrap_or_else(|| "true".to_string());
    for (label, win) in app.webview_windows() {
        if crate::state::is_notch_label(&label) {
            if crate::state::get_flag(crate::state::main_appbar_registered(), &label) {
                register_appbar(win);
            } else if let Ok(hwnd) = win.hwnd() {
                re_assert_topmost(hwnd);
            }
        } else if crate::state::is_dock_label(&label) {
            if dock_enabled == "true" && crate::state::get_flag(crate::state::dock_appbar_registered(), &label) {
                register_dock_appbar(win);
            } else if let Ok(hwnd) = win.hwnd() {
                re_assert_topmost(hwnd);
            }
        }
    }
    sync_overlays(&app);
}

#[tauri::command]
pub async fn change_dock_mode(app: AppHandle, mode: String) {
    // A disabled dock keeps the mode in settings (init_dock applies it on re-enable),
    // just don't show it now.
    let enabled = get_setting_str(&app, "nectar-dock-enabled").unwrap_or_else(|| "true".to_string());
    if enabled != "true" {
        return;
    }
    let labels: Vec<String> = app.webview_windows().keys()
        .filter(|l| crate::state::is_dock_label(l))
        .cloned()
        .collect();
    for label in labels {
        if let Some(dock_win) = app.get_webview_window(&label) {
            change_dock_mode_for_window(&app, dock_win, &mode).await;
        }
    }
}

async fn change_dock_mode_for_window(app: &AppHandle, dock_win: tauri::WebviewWindow, mode: &str) {
    let label = dock_win.label().to_string();
    if mode == "fixed" {
        register_dock_appbar(dock_win.clone());
    } else {
        let _ = dock_win.show();
        if let Ok(hwnd) = dock_win.hwnd() {
            let hwnd_val = hwnd.0 as isize;
            tauri::async_runtime::spawn_blocking(move || {
                unregister_appbar_native(HWND(hwnd_val as *mut _));
            });
            crate::state::set_flag(crate::state::dock_appbar_registered(), &label, false);

            let dock_clone = dock_win.clone();
            tauri::async_runtime::spawn(async move {
                for attempt in 0..10 {
                    // Never fall back to a hardcoded pixel height.
                    // Extract HWND as isize before any await (raw pointer is not Send).
                    let hwnd_val = dock_clone.hwnd().map(|h| h.0 as isize).unwrap_or(0);
                    let ph = dock_clone.outer_size().map(|s| s.height as i32).unwrap_or(0);
                    let monitor_info = crate::monitors::monitor_for_window_label(dock_clone.app_handle(), crate::types::WindowKind::Dock, dock_clone.label()).map(|m| {
                        let s = m.size();
                        let p = m.position();
                        (tauri::PhysicalSize::new(s.width, s.height), tauri::PhysicalPosition::new(p.x, p.y))
                    });

                    if hwnd_val != 0 {
                        if let Some((m_size, m_pos)) = monitor_info {
                            if ph <= 10 {
                                if attempt < 9 {
                                    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
                                }
                                continue;
                            }
                            let final_y = m_pos.y + m_size.height as i32 - ph;
                            unsafe {
                                use windows::Win32::UI::WindowsAndMessaging::{SetWindowPos, SWP_NOZORDER, SWP_NOACTIVATE, SWP_FRAMECHANGED};
                                use windows::Win32::Foundation::HWND;
                                let _ = SetWindowPos(HWND(hwnd_val as *mut _), None, m_pos.x, final_y, m_size.width as i32, ph, SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED);
                            }
                            if let Ok(hwnd) = dock_clone.hwnd() { re_assert_topmost(hwnd); }
                            break;
                        }
                    }
                    if attempt < 9 {
                        tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
                    }
                }
            });
        }
    }

    // Ensure always on top and native taskbar stays hidden
    if let Ok(hwnd) = dock_win.hwnd() { re_assert_topmost(hwnd); }
    set_taskbar_visibility(false, false);
    NATIVE_TASKBAR_HIDDEN.store(true, Ordering::Relaxed);

    // Sync the current overlap state immediately to the frontend
    let current = crate::state::get_overlap(crate::state::dock_overlap(), &label);
    if current != -1 {
        let _ = app.emit_to(label.as_str(), "dock-overlap", current == 1);
    }
    if label == "dock" {
        let _ = app.emit_to(label.as_str(), "dock-maximized", CURRENT_FOREGROUND_MAXIMIZED.load(Ordering::Relaxed));
    }

    // Double sync after a short delay to catch any layout changes
    let dock_clone = dock_win.clone();
    let label_clone = label.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        if crate::state::get_flag(crate::state::dock_appbar_registered(), &label_clone) {
            register_dock_appbar(dock_clone);
        }
    });
}

#[tauri::command]
pub async fn change_notch_mode(app: AppHandle, mode: String) {
    let labels: Vec<String> = app.webview_windows().keys()
        .filter(|l| crate::state::is_notch_label(l))
        .cloned()
        .collect();
    for label in labels {
        if let Some(main_win) = app.get_webview_window(&label) {
            init_notch_window(&app, main_win, &mode).await;
        }
    }
}

pub async fn init_notch_window(app: &AppHandle, main_win: tauri::WebviewWindow, mode: &str) {
    let label = main_win.label().to_string();
    if mode == "fixed" {
        register_appbar(main_win.clone());
    } else {
        let _ = main_win.show();
        if let Ok(hwnd) = main_win.hwnd() {
            let hwnd_val = hwnd.0 as isize;
            tauri::async_runtime::spawn_blocking(move || {
                unregister_appbar_native(HWND(hwnd_val as *mut _));
            });
            crate::state::set_flag(crate::state::main_appbar_registered(), &label, false);

            let main_clone = main_win.clone();
            let label_clone = label.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                if !crate::state::get_flag(crate::state::main_appbar_registered(), &label_clone) {
                    if let Ok(hwnd) = main_clone.hwnd() { re_assert_topmost(hwnd); }
                }
            });
        }
    }
    if let Some(monitor) = crate::monitors::monitor_for_window_label(app, crate::types::WindowKind::Notch, &label) {
        let m_pos = monitor.position();
        let m_size = monitor.size();
        let scale = monitor.scale_factor();
        let nectar_scale = crate::utils::get_nectar_scale(app);
        let target_height = (420.0 * nectar_scale * scale) as u32;
        let _ = main_win.set_position(tauri::PhysicalPosition::new(m_pos.x, m_pos.y));
        let _ = main_win.set_size(tauri::PhysicalSize::new(m_size.width, target_height));
    }

    let current = crate::state::get_overlap(crate::state::notch_overlap(), &label);
    if current != -1 {
        let _ = app.emit_to(label.as_str(), "notch-overlap", current == 1);
    }
}

fn get_uwp_launch_cmd(exe_path: &str) -> Option<String> {
    let path = std::path::Path::new(exe_path);
    let mut is_windows_apps = false;
    let mut package_folder = String::new();
    
    for component in path.components() {
        if let std::path::Component::Normal(s) = component {
            let s_str = s.to_string_lossy();
            if is_windows_apps {
                package_folder = s_str.to_string();
                break;
            }
            if s_str.eq_ignore_ascii_case("WindowsApps") {
                is_windows_apps = true;
            }
        }
    }
    
    if !package_folder.is_empty() {
        // package_folder format: PackageName_Version_Architecture__PublisherId
        // We want: PackageName_PublisherId!App
        if let Some(publisher_idx) = package_folder.rfind("__") {
            let publisher_id = &package_folder[publisher_idx + 2..];
            if let Some(first_underscore) = package_folder.find('_') {
                let package_name = &package_folder[..first_underscore];
                return Some(format!("shell:AppsFolder\\{}_{}!App", package_name, publisher_id));
            }
        }
    }
    None
}

/// Coalesces duplicate events, has to stay below the double-click interval.
const START_TOGGLE_DEBOUNCE_MS: i64 = 80;
/// How long Win is held so the shell registers the tap.
const START_WIN_KEY_HOLD_MS: u64 = 40;

fn send_key_tap(vk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY, hold_ms: u64) {
    use windows::Win32::UI::Input::KeyboardAndMouse::{SendInput, INPUT, INPUT_0, KEYBDINPUT, KEYEVENTF_KEYUP};
    let down = [INPUT {
        r#type: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT { wVk: vk, wScan: 0, dwFlags: Default::default(), time: 0, dwExtraInfo: 0 },
        },
    }];
    let up = [INPUT {
        r#type: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT { wVk: vk, wScan: 0, dwFlags: KEYEVENTF_KEYUP, time: 0, dwExtraInfo: 0 },
        },
    }];
    unsafe {
        SendInput(&down, std::mem::size_of::<INPUT>() as i32);
        if hold_ms > 0 {
            std::thread::sleep(std::time::Duration::from_millis(hold_ms));
        }
        SendInput(&up, std::mem::size_of::<INPUT>() as i32);
    }
}

/// True while the native Start menu has focus (StartMenuExperienceHost, or SearchHost on 24H2+).
fn is_start_menu_open() -> bool {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION};
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

    unsafe {
        let fg = GetForegroundWindow();
        if fg.0.is_null() {
            return false;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(fg, Some(&mut pid));
        if pid == 0 {
            return false;
        }
        if let Ok(process) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
            let mut buf = [0u16; 260];
            let mut len = buf.len() as u32;
            let ok = QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, windows::core::PWSTR(buf.as_mut_ptr()), &mut len);
            let _ = CloseHandle(process);
            if ok.is_ok() {
                let path = String::from_utf16_lossy(&buf[..len as usize]);
                let file = path.rsplit('\\').next().unwrap_or("").to_lowercase();
                return file == "startmenuexperiencehost.exe" || file == "searchhost.exe";
            }
        }
        false
    }
}

/// Toggles the native Start menu. If it's open, Escape closes it, otherwise tap Win.
/// Tapping Win blindly replayed the open animation on a quick second click.
fn toggle_start_menu() {
    let now = get_now_ms();
    if now - LAST_START_TOGGLE_MS.load(Ordering::Relaxed) < START_TOGGLE_DEBOUNCE_MS {
        return;
    }
    LAST_START_TOGGLE_MS.store(now, Ordering::Relaxed);

    tauri::async_runtime::spawn_blocking(move || {
        use windows::Win32::UI::Input::KeyboardAndMouse::{VK_ESCAPE, VK_LWIN};
        if is_start_menu_open() {
            send_key_tap(VK_ESCAPE, 0);
        } else {
            send_key_tap(VK_LWIN, START_WIN_KEY_HOLD_MS);
        }
    });
}

#[tauri::command]
pub async fn open_app(app: AppHandle, app_name: String) {
    if app_name == "start" {
        toggle_start_menu();
        return;
    }

    if app_name == "search" {
        tauri::async_runtime::spawn_blocking(move || unsafe {
            use windows::Win32::UI::Input::KeyboardAndMouse::{SendInput, INPUT, INPUT_0, KEYBDINPUT, VK_LWIN, VK_S, KEYEVENTF_KEYUP};
            let key_down = |vk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY| INPUT {
                r#type: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT { wVk: vk, wScan: 0, dwFlags: Default::default(), time: 0, dwExtraInfo: 0 },
                },
            };
            let key_up = |vk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY| INPUT {
                r#type: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT { wVk: vk, wScan: 0, dwFlags: KEYEVENTF_KEYUP, time: 0, dwExtraInfo: 0 },
                },
            };
            let inputs = [key_down(VK_LWIN), key_down(VK_S), key_up(VK_S), key_up(VK_LWIN)];
            SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
        });
        return;
    }

    if app_name == "nectar-settings" {
        open_settings_window(app);
        return;
    }

    tauri::async_runtime::spawn_blocking(move || launch_path(&app_name));
}

/// Starts another instance of an app. Running PWAs go through their Start Menu
/// shortcut so the new window belongs to the web app.
#[tauri::command]
pub async fn launch_new_instance(app_path: String, app_name: Option<String>) {
    tauri::async_runtime::spawn_blocking(move || {
        if app_path == "start" {
            return;
        }
        let target = pwa_launch_target(&app_path, app_name.as_deref()).unwrap_or(app_path);
        launch_path(&target);
    });
}

/// Finds the Start Menu shortcut for a running PWA (same title, same browser).
fn pwa_launch_target(path: &str, name: Option<&str>) -> Option<String> {
    let name = name?.trim();
    if name.is_empty() || !is_browser_host_process(path) {
        return None;
    }
    let host_exe = std::path::Path::new(path).file_name()?.to_str()?.to_lowercase();
    let apps = INSTALLED_APPS_CACHE.get()?.lock().ok()?;
    apps.iter()
        .find(|app| is_pwa_shortcut_for(app, name, &host_exe))
        .map(|app| app.path.clone())
}

fn is_pwa_shortcut_for(app: &AppInfo, name: &str, host_exe: &str) -> bool {
    app.name.eq_ignore_ascii_case(name)
        && app.executable.as_deref().is_some_and(|exe| exe.eq_ignore_ascii_case(host_exe))
}

/// Opens a path, shortcut or shell app id through the shell.
fn launch_path(path: &str) {
    unsafe {
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
        let wide_open: Vec<u16> = "open".encode_utf16().chain(std::iter::once(0)).collect();

        // Shortcuts and shell app ids go through the shell so args and package identity survive.
        let shell_target = if path.to_lowercase().ends_with(".lnk") {
            Some(path.to_string())
        } else if is_aumid_path(path) {
            let id = path.trim().trim_start_matches("shell:AppsFolder\\");
            Some(format!("shell:AppsFolder\\{}", id))
        } else {
            None
        };

        if let Some(target) = shell_target {
            let wide_target: Vec<u16> = target.encode_utf16().chain(std::iter::once(0)).collect();
            let res = ShellExecuteW(
                None,
                windows::core::PCWSTR(wide_open.as_ptr()),
                windows::core::PCWSTR(wide_target.as_ptr()),
                None,
                None,
                SW_SHOWNORMAL,
            );
            if res.0 as usize <= 32 {
                eprintln!("Failed to open {}: error code {}", target, res.0 as usize);
            }
            return;
        }

        if let Some(uwp_cmd) = crate::commands::get_uwp_launch_cmd(path) {
            let wide_cmd: Vec<u16> = uwp_cmd.encode_utf16().chain(std::iter::once(0)).collect();
            
            let res = ShellExecuteW(
                None,
                windows::core::PCWSTR(wide_open.as_ptr()),
                windows::core::PCWSTR(wide_cmd.as_ptr()),
                None,
                None,
                SW_SHOWNORMAL,
            );
            
            if res.0 as usize <= 32 {
                eprintln!("Failed to open UWP app {}: error code {}", uwp_cmd, res.0 as usize);
            }
            return;
        }

        use std::path::Path;
        let mut final_path = path.to_string();
        
        if !Path::new(&final_path).exists() {
            if let Some(exe_name) = Path::new(&final_path).file_name() {
                if let Some(parent) = Path::new(&final_path).parent().and_then(|p| p.parent()) {
                    if parent.exists() {
                        if let Ok(entries) = std::fs::read_dir(parent) {
                            let mut app_dirs = Vec::new();
                            for entry in entries.flatten() {
                                let name = entry.file_name().to_string_lossy().to_string();
                                if (name.starts_with("app-") || name.starts_with("current")) && entry.path().is_dir() {
                                    app_dirs.push(entry.path());
                                }
                            }
                            app_dirs.sort();
                            if let Some(latest) = app_dirs.last() {
                                let exe = latest.join(exe_name);
                                if exe.exists() {
                                    final_path = exe.to_string_lossy().to_string();
                                }
                            }
                        }
                    }
                }
            }
        }

        let wide_path: Vec<u16> = final_path.encode_utf16().chain(std::iter::once(0)).collect();
        let wide_open: Vec<u16> = "open".encode_utf16().chain(std::iter::once(0)).collect();
        
        let res = ShellExecuteW(
            None,
            windows::core::PCWSTR(wide_open.as_ptr()),
            windows::core::PCWSTR(wide_path.as_ptr()),
            None,
            None,
            SW_SHOWNORMAL,
        );
        
        if res.0 as usize <= 32 {
            eprintln!("Failed to open app {}: error code {}", final_path, res.0 as usize);
        }
    }
}

#[tauri::command]
pub async fn get_active_windows() -> Vec<AppInfo> {
    tauri::async_runtime::spawn_blocking(move || {
        use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};
        // COM is needed here, each window's AppUserModelID comes from the shell property store.
        let com_initialized = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).is_ok() };

        let mut apps: Vec<AppInfo> = Vec::new();
        unsafe {
            let _ = EnumWindows(Some(enum_windows_proc), LPARAM(&mut apps as *mut Vec<AppInfo> as isize));
        }

        let mut grouped: HashMap<String, AppInfo> = HashMap::new();
        
        for app in apps {
            let path = app.path.to_lowercase();
            let name = app.name.to_lowercase();
            
            // For host processes (Edge, Chrome, Brave, ApplicationFrameHost), use path + name 
            // so that different PWAs/UWP apps are separate dock items.
            let key = if path.contains("msedge.exe") || path.contains("chrome.exe") || path.contains("brave.exe") || path.contains("applicationframehost.exe") {
                format!("{}:{}", path, name)
            } else if let Some(ref exe) = app.executable {
                format!("{}:{}", path, exe.to_lowercase())
            } else {
                path.clone()
            };

            if let Some(existing) = grouped.get_mut(&key) {
                if let Some(ref mut hwnds) = existing.all_hwnds {
                    hwnds.push((app.hwnd.unwrap_or(0), app.name.clone()));
                } else {
                    existing.all_hwnds = Some(vec![
                        (existing.hwnd.unwrap_or(0), existing.name.clone()),
                        (app.hwnd.unwrap_or(0), app.name.clone())
                    ]);
                }
            } else {
                let mut new_app = app.clone();
                new_app.all_hwnds = Some(vec![(app.hwnd.unwrap_or(0), app.name.clone())]);
                grouped.insert(key, new_app);
            }
        }

        let focus_guard = if let Some(map) = crate::state::FOCUS_TIMESTAMPS.get() {
            map.lock().ok()
        } else {
            None
        };

        let mut result_apps: Vec<AppInfo> = grouped.into_values().collect();

        for app in &mut result_apps {
            if let Some(ref mut hwnds) = app.all_hwnds {
                hwnds.sort_by(|a, b| {
                    let ts_a = focus_guard.as_ref()
                        .and_then(|g| g.get(&a.0))
                        .copied()
                        .unwrap_or(0);
                    let ts_b = focus_guard.as_ref()
                        .and_then(|g| g.get(&b.0))
                        .copied()
                        .unwrap_or(0);
                    ts_b.cmp(&ts_a)
                });
            }
        }

        if com_initialized {
            unsafe { CoUninitialize(); }
        }

        result_apps
    }).await.unwrap_or_default()
}

unsafe fn force_set_foreground(hwnd: HWND) {
    use windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow;

    let _ = SetForegroundWindow(hwnd);

    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, IsIconic, SwitchToThisWindow};
    if GetForegroundWindow() != hwnd || IsIconic(hwnd).as_bool() {
        SwitchToThisWindow(hwnd, true);
    }
}

static PREVIOUS_FOREGROUND: std::sync::Mutex<Option<(isize, isize)>> = std::sync::Mutex::new(None);
static LAST_FOCUS_CLICK: std::sync::Mutex<Option<(isize, i64)>> = std::sync::Mutex::new(None);

fn is_repeat_click(hwnd: isize) -> bool {
    let now = crate::utils::get_now_ms();
    let Ok(mut guard) = LAST_FOCUS_CLICK.lock() else { return false };
    let repeat = matches!(*guard, Some((last_hwnd, at)) if last_hwnd == hwnd && now - at < 350);
    if !repeat {
        *guard = Some((hwnd, now));
    }
    repeat
}

unsafe fn send_behind(hwnd: HWND) {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindow, GetWindowLongW, GetWindowTextLengthW, GetWindowThreadProcessId, IsIconic, IsWindow, IsWindowVisible,
        GWL_EXSTYLE, GW_HWNDNEXT, WS_EX_TOOLWINDOW,
    };
    let my_pid = std::process::id();
    let usable = |h: HWND| {
        if h == hwnd || !IsWindow(Some(h)).as_bool() || !IsWindowVisible(h).as_bool() || IsIconic(h).as_bool() {
            return false;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(h, Some(&mut pid));
        pid != my_pid
    };

    let previous = PREVIOUS_FOREGROUND.lock().ok().and_then(|g| *g).filter(|(target, _)| *target == hwnd.0 as isize);
    if let Some((_, prev)) = previous {
        let prev = HWND(prev as *mut _);
        if usable(prev) {
            force_set_foreground(prev);
            crate::diagnostics::log(&format!("focus: sent {:?} behind by activating the previous window {:?}", hwnd.0, prev.0));
            return;
        }
    }

    let mut next = GetWindow(hwnd, GW_HWNDNEXT);
    while let Ok(candidate) = next {
        let is_tool = (GetWindowLongW(candidate, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW.0) != 0;
        if usable(candidate) && !is_tool && GetWindowTextLengthW(candidate) > 0 {
            force_set_foreground(candidate);
            crate::diagnostics::log(&format!("focus: sent {:?} behind by activating the next window {:?}", hwnd.0, candidate.0));
            return;
        }
        next = GetWindow(candidate, GW_HWNDNEXT);
    }
    crate::diagnostics::log(&format!("focus: found no window to activate behind {:?}", hwnd.0));
}

#[tauri::command]
pub async fn focus_window(hwnd: isize) {
    if is_repeat_click(hwnd) {
        return;
    }
    tauri::async_runtime::spawn_blocking(move || unsafe {
        use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_RESTORE, SW_SHOW, SW_MINIMIZE, IsIconic, IsWindowVisible, GetForegroundWindow, GetWindowThreadProcessId};
        let hwnd = HWND(hwnd as *mut _);
        let my_pid = std::process::id();

        let foreground_before = GetForegroundWindow();
        if !foreground_before.is_invalid() && foreground_before != hwnd {
            let mut before_pid = 0u32;
            GetWindowThreadProcessId(foreground_before, Some(&mut before_pid));
            if before_pid != my_pid {
                if let Ok(mut guard) = PREVIOUS_FOREGROUND.lock() {
                    *guard = Some((hwnd.0 as isize, foreground_before.0 as isize));
                }
            }
        }

        if !IsWindowVisible(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = ShowWindow(hwnd, SW_RESTORE);
            force_set_foreground(hwnd);
        } else if IsIconic(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
            force_set_foreground(hwnd);
        } else {
            let fg = GetForegroundWindow();
            let mut should_minimize = false;

            if !fg.is_invalid() && fg == hwnd {
                should_minimize = true;
            } else if !fg.is_invalid() {
                let mut fg_pid = 0u32;
                let mut target_pid = 0u32;
                GetWindowThreadProcessId(fg, Some(&mut fg_pid));
                GetWindowThreadProcessId(hwnd, Some(&mut target_pid));
                if fg_pid == target_pid && fg_pid != 0 && fg_pid != my_pid {
                    should_minimize = true;
                }
            }

            if !should_minimize {
                let now = crate::utils::get_now_ms();
                should_minimize = if let Some(map) = crate::state::FOCUS_TIMESTAMPS.get() {
                    if let Ok(guard) = map.lock() {
                        guard.get(&(hwnd.0 as *mut () as isize))
                            .map(|ts| now - ts < 2000)
                            .unwrap_or(false)
                    } else { false }
                } else { false };
            }

            if should_minimize {
                let _ = ShowWindow(hwnd, SW_MINIMIZE);
                if !IsIconic(hwnd).as_bool() {
                    use windows::Win32::UI::WindowsAndMessaging::{SendMessageTimeoutW, SC_MINIMIZE, SMTO_ABORTIFHUNG, WM_SYSCOMMAND};
                    let _ = SendMessageTimeoutW(
                        hwnd,
                        WM_SYSCOMMAND,
                        windows::Win32::Foundation::WPARAM(SC_MINIMIZE as usize),
                        windows::Win32::Foundation::LPARAM(0),
                        SMTO_ABORTIFHUNG,
                        300,
                        None,
                    );
                }
                let minimized = IsIconic(hwnd).as_bool();
                crate::diagnostics::log(&format!("focus: minimize of {:?} worked: {minimized}", hwnd.0));
                if !minimized {
                    send_behind(hwnd);
                }
            } else {
                force_set_foreground(hwnd);
            }
        }
    }).await.unwrap_or_default();
}

/// Focuses one window of a multi-window item. hwnds are most recent first: if none is
/// in front take the first, otherwise go to the next one so clicks cycle through all of them.
#[tauri::command]
pub async fn focus_app_windows(hwnds: Vec<isize>) {
    if let Some(&first) = hwnds.first() {
        if is_repeat_click(first) {
            return;
        }
    }
    tauri::async_runtime::spawn_blocking(move || unsafe {
        use windows::Win32::UI::WindowsAndMessaging::{
            GetForegroundWindow, IsIconic, IsWindowVisible, SetForegroundWindow, ShowWindow,
            SW_RESTORE, SW_SHOW,
        };
        let Some(&most_recent) = hwnds.first() else {
            return;
        };
        let foreground = GetForegroundWindow().0 as isize;
        let mut ring = hwnds.clone();
        ring.sort_unstable();
        let target = match ring.iter().position(|&h| h == foreground) {
            Some(i) => ring[(i + 1) % ring.len()],
            None => most_recent,
        };

        let hwnd = HWND(target as *mut _);
        if !IsWindowVisible(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_SHOW);
        }
        if IsIconic(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }
        let _ = SetForegroundWindow(hwnd);
    })
    .await
    .unwrap_or_default();
}

fn get_cache_key(path: &str, name: Option<&str>) -> String {
    let path_lc = path.to_lowercase();
    let name_lc = name.map(|n| n.to_lowercase()).unwrap_or_default();
    if path_lc.contains("msedge.exe") || path_lc.contains("chrome.exe") || path_lc.contains("brave.exe") || path_lc.contains("applicationframehost.exe") {
        format!("{}:{}", path, name_lc)
    } else {
        path.to_string()
    }
}

fn get_custom_icons_dir(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?.join("custom_icons");
    let _ = std::fs::create_dir_all(&dir);
    Ok(dir)
}

fn sanitize_filename(key: &str) -> String {
    key.replace(|c: char| c == ':' || c == '\\' || c == '/' || c == '*' || c == '?' || c == '"' || c == '<' || c == '>' || c == '|', "_")
}

/// Reads a --flag=value argument from a command line, handles quotes.
fn extract_arg(args: &str, key: &str) -> Option<String> {
    let idx = args.find(key)?;
    let rest = &args[idx + key.len()..];
    let value = if let Some(stripped) = rest.strip_prefix('"') {
        stripped.split('"').next().unwrap_or("")
    } else {
        rest.split_whitespace().next().unwrap_or("")
    };
    let value = value.trim().trim_matches('"');
    if value.is_empty() { None } else { Some(value.to_string()) }
}

/// True for shell app ids (PackageFamily!App etc.), not file paths.
pub fn is_aumid_path(path: &str) -> bool {
    let p = path.trim().trim_matches('"');
    if p.is_empty() || p.len() < 3 { return false; }
    if p.to_lowercase().starts_with("shell:appsfolder\\") { return true; }
    if p.contains('\\') || p.contains('/') || p.contains(':') { return false; }
    if p.to_lowercase().ends_with(".exe") { return false; }
    if std::path::Path::new(p).exists() { return false; }
    true
}

fn is_browser_host_process(path: &str) -> bool {
    let p = path.to_lowercase();
    p.contains("msedge.exe") || p.contains("chrome.exe") || p.contains("brave.exe") || p.contains("vivaldi.exe") || p.contains("firefox.exe")
}

/// Icon for a shell app id via shell:AppsFolder (SHGetFileInfoW can't do these).
fn icon_from_aumid(aumid: &str) -> Option<String> {
    unsafe {
        use windows::Win32::Foundation::SIZE;
        use windows::Win32::UI::Shell::{IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF, SIIGBF_BIGGERSIZEOK, SIIGBF_ICONONLY};

        let id = aumid.trim().trim_start_matches("shell:AppsFolder\\");
        if id.is_empty() { return None; }

        let parsing_name = format!("shell:AppsFolder\\{}", id);
        let wide: Vec<u16> = parsing_name.encode_utf16().chain(std::iter::once(0)).collect();
        let factory: IShellItemImageFactory = SHCreateItemFromParsingName(windows::core::PCWSTR(wide.as_ptr()), None).ok()?;

        let size = SIZE { cx: 256, cy: 256 };
        let hbitmap = factory.GetImage(size, SIIGBF(SIIGBF_ICONONLY.0 | SIIGBF_BIGGERSIZEOK.0))
            .or_else(|_| factory.GetImage(size, SIIGBF_BIGGERSIZEOK))
            .ok()?;
        if hbitmap.0.is_null() { return None; }

        let result = crate::utils::hbitmap_to_base64(hbitmap);
        let _ = windows::Win32::Graphics::Gdi::DeleteObject(hbitmap.into());
        result
    }
}

/// Package family name (Name_PublisherId) from a WindowsApps exe path.
fn package_family_from_windows_apps_path(path: &str) -> Option<String> {
    let lower = path.to_lowercase();
    let idx = lower.find("\\windowsapps\\")?;
    let rest = &path[idx + "\\windowsapps\\".len()..];
    let folder = rest.split(['\\', '/']).next()?;
    let (before_publisher, publisher) = folder.rsplit_once("__")?;
    let name = before_publisher.split('_').next()?;
    if name.is_empty() || publisher.is_empty() { return None; }
    Some(format!("{}_{}", name, publisher))
}

/// Looks up the exact AUMID of a package family by walking shell:AppsFolder.
fn find_aumid_by_family(family: &str) -> Option<String> {
    use windows::Win32::System::Com::CoTaskMemFree;
    use windows::Win32::UI::Shell::{SHGetKnownFolderIDList, FOLDERID_AppsFolder, SHGetDesktopFolder, IShellFolder, IEnumIDList, SHGetNameFromIDList, ILCombine, ILFree, SIGDN_DESKTOPABSOLUTEPARSING, SHCONTF_FOLDERS, SHCONTF_NONFOLDERS};

    unsafe {
        let pidl_apps = SHGetKnownFolderIDList(&FOLDERID_AppsFolder, 0, None).ok()?;
        let result = (|| -> Option<String> {
            let desktop = SHGetDesktopFolder().ok()?;
            let apps_folder: IShellFolder = desktop.BindToObject(pidl_apps, None).ok()?;
            let mut enum_id: Option<IEnumIDList> = None;
            if apps_folder.EnumObjects(HWND(std::ptr::null_mut()), (SHCONTF_FOLDERS.0 | SHCONTF_NONFOLDERS.0) as u32, &mut enum_id).is_err() { return None; }
            let enum_id = enum_id?;

            let prefix = format!("{}!", family);
            let mut pidl_buf: [*mut windows::Win32::UI::Shell::Common::ITEMIDLIST; 1] = [std::ptr::null_mut()];
            let mut fetched = 0;
            while enum_id.Next(&mut pidl_buf, Some(&mut fetched)).is_ok() && fetched > 0 {
                let pidl_item = pidl_buf[0];
                pidl_buf[0] = std::ptr::null_mut();
                if pidl_item.is_null() { continue; }

                let absolute_pidl = ILCombine(Some(pidl_apps as *const _), Some(pidl_item as *const _));
                if absolute_pidl.is_null() {
                    CoTaskMemFree(Some(pidl_item as *const _));
                    continue;
                }

                let mut found = None;
                if let Ok(p_ptr) = SHGetNameFromIDList(absolute_pidl, SIGDN_DESKTOPABSOLUTEPARSING) {
                    let s = String::from_utf16_lossy(windows::core::PCWSTR(p_ptr.0).as_wide());
                    CoTaskMemFree(Some(p_ptr.0 as *const _));
                    if s.starts_with(&prefix) { found = Some(s); }
                }
                ILFree(Some(absolute_pidl as *const _));
                CoTaskMemFree(Some(pidl_item as *const _));
                if found.is_some() { return found; }
            }
            None
        })();
        CoTaskMemFree(Some(pidl_apps as *const _));
        result
    }
}

/// Icon for exes inside an MSIX package. The exe has none, so use the manifest logo.
fn packaged_exe_icon(path: &str) -> Option<String> {
    let family = package_family_from_windows_apps_path(path)?;
    let aumid = find_aumid_by_family(&family)?;
    icon_from_aumid(&aumid)
}

/// Process command line via WMI, used to spot PWAs inside browser processes.
fn process_command_line(pid: u32) -> Option<String> {
    use serde::Deserialize;
    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct Win32Process {
        command_line: Option<String>,
    }

    let com = wmi::COMLibrary::new().ok()?;
    let connection = wmi::WMIConnection::new(com).ok()?;
    let results: Vec<Win32Process> = connection
        .raw_query(format!("SELECT CommandLine FROM Win32_Process WHERE ProcessId = {}", pid))
        .ok()?;
    results.into_iter().next().and_then(|p| p.command_line)
}

fn pwa_icon_from_command_line(process_path: &str, command_line: &str) -> Option<String> {
    // Store PWAs from Edge carry their package id here.
    if let Some(aumid) = extract_arg(command_line, "--ip-aumid=") {
        if let Some(icon) = icon_from_aumid(&aumid) { return Some(icon); }
    }
    // Installed web app windows carry the web app id.
    if command_line.contains("--app-id=") {
        if let Some(icon_path) = find_browser_pwa_icon(process_path, command_line) {
            if let Some(icon) = crate::utils::image_file_to_base64(&icon_path) { return Some(icon); }
        }
    }
    None
}

/// Web app ids are 32 chars, a-p.
pub(crate) fn browser_web_app_id_from_aumid(id: &str) -> Option<String> {
    let is_web_app_id = |s: &str| s.len() == 32 && s.bytes().all(|b| (b'a'..=b'p').contains(&b));
    if let Some(segment) = id.rsplit('.').next() {
        if is_web_app_id(segment) { return Some(segment.to_string()); }
    }
    if let Some(idx) = id.find("_crx_") {
        if let Some(segment) = id[idx + 5..].split('.').next() {
            if is_web_app_id(segment) { return Some(segment.to_string()); }
        }
    }
    None
}

/// True if an AUMID belongs to a browser-hosted app (PWA) and not the browser itself.
/// Brave uses a shortened id, so the _crx_ marker is checked directly.
pub(crate) fn is_browser_pwa_aumid(id: &str) -> bool {
    id.contains('!') || id.contains("_crx_") || browser_web_app_id_from_aumid(id).is_some()
}

unsafe fn pwa_icon_for_window(hwnd: HWND, process_path: &str) -> Option<String> {
    // The window's own AUMID is the most reliable, the command line isn't (Edge reuses its browser process).
    match crate::utils::get_window_app_user_model_id(hwnd) {
        Some(id) if id.contains('!') => {
            if let Some(icon) = icon_from_aumid(&id) { return Some(icon); }
        }
        Some(id) if is_browser_pwa_aumid(&id) => {
            // PWA window: app id from the AUMID, else from the command line.
            let mut pid = 0u32;
            windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if pid == 0 { return None; }
            if let Some(app_id) = browser_web_app_id_from_aumid(&id) {
                if let Some(icon_path) = find_browser_pwa_icon(process_path, &format!("--app-id={}", app_id)) {
                    if let Some(icon) = crate::utils::image_file_to_base64(&icon_path) { return Some(icon); }
                }
            }
            if let Some(command_line) = process_command_line(pid) {
                if let Some(icon) = pwa_icon_from_command_line(process_path, &command_line) { return Some(icon); }
            }
            return None;
        }
        // Plain browser window. Its process command line can carry an --app-id from another
        // window, so skip it and let the caller use the window icon.
        Some(_) => return None,
        None => {}
    }
    // Fall back to the command line when the window has no id.
    let mut pid = 0u32;
    windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(hwnd, Some(&mut pid));
    if pid == 0 { return None; }
    let command_line = process_command_line(pid)?;
    pwa_icon_from_command_line(process_path, &command_line)
}

/// Finds a web app's icon in the browser profile (Manifest Resources/<id>/Icons,
/// or the old _crx_ layout).
fn find_browser_pwa_icon(executable_path: &str, args: &str) -> Option<String> {
    let app_id = extract_arg(args, "--app-id=")?;
    let local = std::env::var("LOCALAPPDATA").ok()?;

    let chrome_base = format!("{}\\Google\\Chrome\\User Data", local);
    let edge_base = format!("{}\\Microsoft\\Edge\\User Data", local);
    let brave_base = format!("{}\\BraveSoftware\\Brave-Browser\\User Data", local);
    let vivaldi_base = format!("{}\\Vivaldi\\User Data", local);
    let browser_path = executable_path.to_lowercase();
    let bases = if browser_path.contains("msedge") {
        vec![edge_base, chrome_base]
    } else if browser_path.contains("brave") {
        vec![brave_base, chrome_base]
    } else if browser_path.contains("vivaldi") {
        vec![vivaldi_base, chrome_base]
    } else {
        vec![chrome_base, edge_base]
    };

    let profile = extract_arg(args, "--profile-directory=").unwrap_or_else(|| "Default".to_string());

    for base in bases {
        if !std::path::Path::new(&base).is_dir() { continue; }

        let mut profiles = vec![profile.clone()];
        if let Ok(entries) = std::fs::read_dir(&base) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if !profiles.iter().any(|p| p.eq_ignore_ascii_case(&name)) { profiles.push(name); }
                }
            }
        }

        for prof in profiles {
            let web_apps = std::path::Path::new(&base).join(&prof).join("Web Applications");
            if !web_apps.is_dir() { continue; }

            let candidates = [
                web_apps.join("Manifest Resources").join(&app_id),
                web_apps.join(&app_id),
                web_apps.join(format!("_crx_{}", app_id)),
            ];
            for dir in candidates {
                if let Some(icon) = find_largest_image_in_dir(&dir) { return Some(icon); }
            }
        }
    }
    None
}

fn find_largest_image_in_dir(dir: &std::path::Path) -> Option<String> {
    if !dir.is_dir() { return None; }
    let mut best: Option<(u64, String)> = None;
    collect_images(dir, 0, &mut best);
    best.map(|(_, path)| path)
}

fn collect_images(dir: &std::path::Path, depth: u32, best: &mut Option<(u64, String)>) {
    if depth > 3 { return; }
    let Ok(entries) = std::fs::read_dir(dir) else { return; };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_images(&path, depth + 1, best);
            continue;
        }
        let Some(ext) = path.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()) else { continue; };
        if !matches!(ext.as_str(), "png" | "ico" | "jpg" | "jpeg" | "bmp" | "webp") { continue; }
        let score = image_size_score(&path);
        let is_better = match best.as_ref() {
            Some((best_score, _)) => score > *best_score,
            None => true,
        };
        if is_better {
            *best = Some((score, path.to_string_lossy().to_string()));
        }
    }
}

/// Scores an icon by the size in its file name (256.png, 192x192.png).
fn image_size_score(path: &std::path::Path) -> u64 {
    let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
    let mut score: u64 = 0;
    for token in name.split(|c: char| !c.is_ascii_digit() && c != 'x') {
        if token.is_empty() { continue; }
        if let Some(idx) = token.find('x') {
            let (w, h) = token.split_at(idx);
            let h = &h[1..];
            if let (Ok(w), Ok(h)) = (w.parse::<u64>(), h.parse::<u64>()) {
                score = score.max(w * h);
            }
        } else if let Ok(n) = token.parse::<u64>() {
            score = score.max(n * n);
        }
    }
    score
}

static LAST_ICON_SAVE_REQUEST: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);

pub fn schedule_icon_cache_save(app: AppHandle) {
    let now = crate::utils::get_now_ms();
    LAST_ICON_SAVE_REQUEST.store(now, Ordering::Relaxed);

    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
        let last = LAST_ICON_SAVE_REQUEST.load(Ordering::Relaxed);
        if now == last || crate::utils::get_now_ms() - last >= 950 {
            if let Ok(dir) = app.path().app_config_dir() {
                let cache_path = dir.join("icons_cache.json");
                if let Some(c) = crate::state::ICON_CACHE.get() {
                    let snapshot = if let Ok(lock) = c.lock() {
                        serde_json::to_string(&*lock).unwrap_or_default()
                    } else {
                        String::new()
                    };
                    if !snapshot.is_empty() {
                        let _ = tauri::async_runtime::spawn_blocking(move || {
                            let _ = std::fs::write(&cache_path, snapshot);
                        }).await;
                    }
                }
            }
        }
    });
}

#[tauri::command]
pub async fn get_app_icon(app: AppHandle, path: String, name: Option<String>, hwnd: Option<isize>) -> Result<Option<String>, String> {
    let cache_dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    let cache_path = cache_dir.join("icons_cache.json");
    let cache_key = get_cache_key(&path, name.as_deref());

    if path == "nectar-settings" {
        if let Some(icon) = app.default_window_icon() {
            if let Some(rgba) = image::RgbaImage::from_raw(icon.width(), icon.height(), icon.rgba().to_vec()) {
                let mut out = Vec::new();
                if image::DynamicImage::ImageRgba8(rgba).write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).is_ok() {
                    use base64::Engine;
                    let b64 = base64::engine::general_purpose::STANDARD.encode(&out);
                    return Ok(Some(format!("data:image/png;base64,{}", b64)));
                }
            }
        }
    }

    // Strategy 0: Check for custom icon first
    let custom_icons_dir = cache_dir.join("custom_icons");
    let custom_file = custom_icons_dir.join(format!("{}.png", sanitize_filename(&cache_key)));
    if custom_file.exists() {
        if let Ok(data) = std::fs::read(&custom_file) {
            use base64::Engine;
            let b64 = base64::engine::general_purpose::STANDARD.encode(&data);
            return Ok(Some(format!("data:image/png;base64,{}", b64)));
        }
    }

    let cache = ICON_CACHE.get_or_init(|| {
        let version_marker = cache_dir.join("icons_cache_v2");
        if !version_marker.exists() {
            let _ = std::fs::remove_file(&cache_path);
            let _ = std::fs::create_dir_all(&cache_dir);
            let _ = std::fs::write(&version_marker, b"1");
        }
        let mut map = std::collections::HashMap::new();
        if let Ok(content) = std::fs::read_to_string(&cache_path) {
            if let Ok(existing) = serde_json::from_str::<std::collections::HashMap<String, String>>(&content) {
                map = existing;
            }
        }
        std::sync::Mutex::new(map)
    });

    let is_uwp_host = path.to_lowercase().contains("applicationframehost.exe");
    let stale_store_path = path.to_lowercase().contains("\\windowsapps\\") && !std::path::Path::new(&path).exists();
    let match_by_name = is_uwp_host || stale_store_path;
    let lookup_name: Option<String> = name.clone().or_else(|| {
        if stale_store_path {
            std::path::Path::new(&path).file_stem().map(|s| s.to_string_lossy().to_string())
        } else {
            None
        }
    });

    if match_by_name {
        let apps_cache = INSTALLED_APPS_CACHE.get_or_init(|| std::sync::Mutex::new(Vec::new()));
        let empty = apps_cache.lock().map(|a| a.is_empty()).unwrap_or(true);
        if empty && !IS_SCANNING.load(Ordering::Relaxed) { crate::services::trigger_app_scan(); }
        let start = std::time::Instant::now();
        while IS_SCANNING.load(Ordering::Relaxed) && start.elapsed() < std::time::Duration::from_secs(10) {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    }

    if let Some(apps_cache) = INSTALLED_APPS_CACHE.get() {
        if let Ok(apps) = apps_cache.lock() {
            let scanned = apps.iter().find(|a| {
                a.icon.is_some()
                    && (a.path == path
                        || (match_by_name && lookup_name.as_deref().is_some_and(|n| a.name.eq_ignore_ascii_case(n))))
            });
            if let Some(scanned) = scanned {
                let icon = scanned.icon.clone().unwrap();
                if let Ok(mut c) = cache.lock() {
                    c.insert(cache_key.clone(), icon.clone());
                }
                schedule_icon_cache_save(app.clone());
                return Ok(Some(icon));
            }
        }
    }

    {
        let path_for_pin = path.clone();
        let pin_icon = tauri::async_runtime::spawn_blocking(move || unsafe {
            use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED, CoUninitialize};
            let com = CoInitializeEx(None, COINIT_MULTITHREADED).is_ok();
            let icon = crate::taskbar_pins::icon_for_target(&path_for_pin);
            if com { CoUninitialize(); }
            icon
        }).await.unwrap_or(None);

        if let Some(icon) = pin_icon {
            if let Ok(mut c) = cache.lock() { c.insert(cache_key.clone(), icon.clone()); }
            schedule_icon_cache_save(app.clone());
            return Ok(Some(icon));
        }
    }

    if let Ok(c) = cache.lock() {
        if let Some(icon) = c.get(&cache_key) {
            return Ok(Some(icon.clone()));
        }
    }

    // Resolve bare names (notepad.exe) first so Store packages get their own logo, not the System32 stub.
    let mut path = path;
    if !path.contains('\\') && !path.contains('/') {
        if let Some(resolved) = resolve_executable_path(&path) {
            path = resolved;
        }
    }

    // Strategy 2: shell-resolved icons for Store/UWP/PWA ids and exes inside MSIX packages.
    let aumid_opt = if is_aumid_path(&path) {
        Some(path.trim().trim_start_matches("shell:AppsFolder\\").to_string())
    } else {
        None
    };
    let packaged_exe = path.to_lowercase().contains("\\windowsapps\\");
    if aumid_opt.is_some() || packaged_exe {
        let path_owned = path.clone();
        let result = tauri::async_runtime::spawn_blocking(move || {
            let _ = unsafe { windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_MULTITHREADED) };
            let icon = match aumid_opt {
                Some(aumid) => icon_from_aumid(&aumid),
                None => packaged_exe_icon(&path_owned),
            };
            unsafe { windows::Win32::System::Com::CoUninitialize(); }
            icon
        }).await.unwrap_or(None);

        if let Some(base64) = result {
            if let Ok(mut c) = cache.lock() { c.insert(cache_key.clone(), base64.clone()); }
            schedule_icon_cache_save(app);
            return Ok(Some(base64));
        }
    }

    // Strategy 3: Extract icon from live window HWND
    if let Some(h) = hwnd {
        let path_owned = path.clone();
        let result = tauri::async_runtime::spawn_blocking(move || unsafe {
            use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED, CoUninitialize};
            use windows::Win32::UI::WindowsAndMessaging::{GetClassLongPtrW, GCLP_HICON, WM_GETICON, ICON_BIG, SendMessageTimeoutW, SMTO_ABORTIFHUNG};
            
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);

            // Browser-hosted PWAs only show the browser icon on the window, so go by the
            // window's AUMID (command line as fallback).
            if is_browser_host_process(&path_owned) {
                if let Some(icon) = pwa_icon_for_window(HWND(h as *mut _), &path_owned) {
                    CoUninitialize();
                    return Some(icon);
                }
            }

            let h_hwnd = HWND(h as *mut _);
            
            let mut h_icon = windows::Win32::UI::WindowsAndMessaging::HICON(GetClassLongPtrW(h_hwnd, GCLP_HICON) as *mut _);
            if h_icon.is_invalid() {
                h_icon = windows::Win32::UI::WindowsAndMessaging::HICON(GetClassLongPtrW(h_hwnd, windows::Win32::UI::WindowsAndMessaging::GCL_HICON) as *mut _);
            }

            if h_icon.is_invalid() {
                let mut res = 0usize;
                let _ = SendMessageTimeoutW(h_hwnd, WM_GETICON, windows::Win32::Foundation::WPARAM(ICON_BIG as usize), windows::Win32::Foundation::LPARAM(0), SMTO_ABORTIFHUNG, 250, Some(&mut res));
                if res != 0 { h_icon = windows::Win32::UI::WindowsAndMessaging::HICON(res as *mut _); }
            }
            
            let res = if !h_icon.is_invalid() { icon_to_base64(h_icon) } else { None };
            
            CoUninitialize();
            res
        }).await.unwrap_or(None);

        if let Some(base64) = result {
            if let Ok(mut c) = cache.lock() {
                c.insert(cache_key.clone(), base64.clone());
            }
            schedule_icon_cache_save(app);
            return Ok(Some(base64));
        }
    }

    // Strategy 4: Extract icon from file path
    let path_clone = path.clone();
    let ck_clone = cache_key.clone();
    let file_icon = tauri::async_runtime::spawn_blocking(move || unsafe {
        use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED, CoUninitialize};
        use windows::Win32::UI::Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON};
        use windows::Win32::UI::WindowsAndMessaging::DestroyIcon;
        
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let result = {
            let (mut actual_path, args) = if path_clone.to_lowercase().ends_with(".lnk") {
                resolve_shortcut(&path_clone).unwrap_or((path_clone.clone(), String::new()))
            } else {
                (path_clone.clone(), String::new())
            };

            // Shortcuts can have their own icon file (Firefox web apps).
            let mut icon_data = if path_clone.to_lowercase().ends_with(".lnk") {
                get_shortcut_icon_location(&path_clone).and_then(|p| image_file_to_base64(&p))
            } else {
                None
            };

            // Web apps keep their icon in the browser profile.
            if icon_data.is_none() {
                if let Some(pwa_icon_path) = find_browser_pwa_icon(&actual_path, &args) {
                    icon_data = image_file_to_base64(&pwa_icon_path);
                }
            }

            // UWP shortcuts launch `explorer.exe shell:AppsFolder\<AUMID>`.
            if icon_data.is_none() {
                if let Some(aumid) = extract_arg(&args, "shell:AppsFolder\\") {
                    icon_data = icon_from_aumid(&aumid);
                }
            }

            if let Some(icon) = icon_data {
                if let Ok(mut lock) = ICON_CACHE.get().unwrap().lock() { lock.insert(ck_clone, icon.clone()); }
                CoUninitialize();
                return Some(icon);
            }

            // Robust path resolution for common apps
            if !std::path::Path::new(&actual_path).is_absolute() {
                let lower = actual_path.to_lowercase();
                if lower == "code" || lower == "code.exe" {
                    if let Ok(home) = std::env::var("USERPROFILE") {
                        let p = format!("{}\\AppData\\Local\\Programs\\Microsoft VS Code\\Code.exe", home);
                        if std::path::Path::new(&p).exists() { actual_path = p; }
                    }
                } else if lower == "msedge" || lower == "msedge.exe" {
                    let candidates = [
                        r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
                        r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
                    ];
                    for c in &candidates {
                        if std::path::Path::new(c).exists() { actual_path = c.to_string(); break; }
                    }
                } else if lower == "notepad" || lower == "notepad.exe" {
                    let candidates = [
                        r"C:\Windows\System32\notepad.exe",
                        r"C:\Windows\notepad.exe",
                    ];
                    for c in &candidates {
                        if std::path::Path::new(c).exists() { actual_path = c.to_string(); break; }
                    }
                } else if lower == "explorer" || lower == "explorer.exe" {
                    let p = r"C:\Windows\explorer.exe";
                    if std::path::Path::new(p).exists() { actual_path = p.to_string(); }
                }
            }

            if actual_path.to_lowercase().starts_with("shell:") {
                if let Some(icon) = crate::utils::icon_from_parsing_name(&actual_path) {
                    if let Ok(mut lock) = ICON_CACHE.get().unwrap().lock() { lock.insert(ck_clone, icon.clone()); }
                    CoUninitialize();
                    return Some(icon);
                }
            }

            let mut shfi: SHFILEINFOW = std::mem::zeroed();
            let icon_path = &actual_path;
            let path_u16: Vec<u16> = icon_path.encode_utf16().chain(std::iter::once(0)).collect();
            let res = SHGetFileInfoW(windows::core::PCWSTR(path_u16.as_ptr()), Default::default(), Some(&mut shfi), std::mem::size_of::<SHFILEINFOW>() as u32, SHGFI_ICON | SHGFI_LARGEICON);

            if res != 0 && !shfi.hIcon.is_invalid() {
                let base64_icon = icon_to_base64(shfi.hIcon);
                let _ = DestroyIcon(shfi.hIcon);
                if let Some(ref base64) = base64_icon {
                    if let Ok(mut lock) = ICON_CACHE.get().unwrap().lock() { lock.insert(ck_clone, base64.clone()); }
                }
                Some(base64_icon)
            } else { None }
        };
        CoUninitialize();
        result.flatten()
    }).await.map_err(|e| e.to_string())?;

    if file_icon.is_some() {
        schedule_icon_cache_save(app);
    }
    Ok(file_icon)
}

#[tauri::command]
pub fn save_pinned_apps(app: AppHandle, apps: Vec<AppInfo>) -> Result<(), String> {
    let path = app.path().app_config_dir().map_err(|e| e.to_string())?.join("pinned_apps.json");
    if let Some(parent) = path.parent() { let _ = std::fs::create_dir_all(parent); }
    let content = serde_json::to_string(&apps).map_err(|e| e.to_string())?;
    std::fs::write(path, content).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn load_pinned_apps(app: AppHandle) -> Vec<AppInfo> {
    let cfg_dir = app.path().app_config_dir().unwrap_or_default();
    let path = cfg_dir.join("pinned_apps.json");
    let synced_marker = cfg_dir.join("taskbar_pins_synced");

    if !synced_marker.exists() {
        let imported = tauri::async_runtime::spawn_blocking(import_taskbar_pins).await.ok().flatten();
        if let Some(imported) = imported {
            if !imported.is_empty() {
                let _ = save_pinned_apps(app.clone(), imported.clone());
                let _ = std::fs::write(&synced_marker, b"1");
                return imported;
            }
        }
    }

    if let Ok(content) = std::fs::read_to_string(&path) {
        if let Ok(apps) = serde_json::from_str(&content) { return apps; }
    }

    vec![
        AppInfo { name: "File Explorer".into(), path: "C:\\Windows\\explorer.exe".into(), icon: None, is_running: false, hwnd: None, executable: Some("explorer.exe".into()), all_hwnds: None },
        AppInfo { name: "Microsoft Edge".into(), path: "msedge".into(), icon: None, is_running: false, hwnd: None, executable: Some("msedge.exe".into()), all_hwnds: None },
        AppInfo { name: "Notepad".into(), path: "notepad.exe".into(), icon: None, is_running: false, hwnd: None, executable: Some("notepad.exe".into()), all_hwnds: None },
        AppInfo { name: "Settings".into(), path: "nectar-settings".into(), icon: None, is_running: false, hwnd: None, executable: None, all_hwnds: None },
    ]
}

fn import_taskbar_pins() -> Option<Vec<AppInfo>> {
    use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};
    unsafe {
        let com = CoInitializeEx(None, COINIT_MULTITHREADED).is_ok();
        let pins = crate::taskbar_pins::read_pins();
        if com { CoUninitialize(); }
        pins
    }
}

#[tauri::command]
pub async fn clear_icon_cache(app: AppHandle) -> Result<(), String> {
    let cache_dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    let cache_path = cache_dir.join("icons_cache.json");
    let _ = std::fs::remove_file(&cache_path);
    if let Some(c) = ICON_CACHE.get() {
        if let Ok(mut lock) = c.lock() {
            lock.clear();
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn set_custom_icon(app: AppHandle, cache_key: String, icon_data: String) -> Result<String, String> {
    use base64::Engine;
    use image::GenericImageView;

    // Validate and decode the base64 data URI
    let b64_str = if icon_data.starts_with("data:") {
        icon_data.split(',').nth(1).ok_or("Invalid data URI")?
    } else {
        &icon_data
    };

    let raw_bytes = base64::engine::general_purpose::STANDARD
        .decode(b64_str)
        .map_err(|e| format!("Invalid base64: {}", e))?;

    // Decode image to validate and get dimensions
    let img = image::load_from_memory(&raw_bytes)
        .map_err(|e| format!("Invalid image: {}", e))?;

    let (w, h) = img.dimensions();
    if w < 16 || h < 16 {
        return Err("Icon must be at least 16x16 pixels".into());
    }

    // Resize to fit within 256x256 preserving aspect ratio, then center on transparent canvas
    let target = 256u32;
    let final_img = if w > target || h > target {
        let scaled = img.resize(target, target, image::imageops::FilterType::Lanczos3);
        let (sw, sh) = scaled.dimensions();
        let mut canvas = image::RgbaImage::new(target, target);
        let ox = (target - sw) / 2;
        let oy = (target - sh) / 2;
        image::imageops::overlay(&mut canvas, &scaled, ox as i64, oy as i64);
        image::DynamicImage::ImageRgba8(canvas)
    } else {
        img
    };

    // Encode to PNG bytes
    let mut png_bytes: Vec<u8> = Vec::new();
    final_img.write_to(&mut std::io::Cursor::new(&mut png_bytes), image::ImageFormat::Png)
        .map_err(|e| format!("Failed to encode PNG: {}", e))?;

    // Save to custom_icons directory
    let icons_dir = get_custom_icons_dir(&app)?;
    let filename = format!("{}.png", sanitize_filename(&cache_key));
    let file_path = icons_dir.join(&filename);
    std::fs::write(&file_path, &png_bytes).map_err(|e| e.to_string())?;

    // Update manifest mapping sanitized filename -> original cache key
    let manifest_path = icons_dir.join("custom_icons.json");
    let mut manifest: std::collections::HashMap<String, String> = if let Ok(content) = std::fs::read_to_string(&manifest_path) {
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        std::collections::HashMap::new()
    };
    manifest.insert(sanitize_filename(&cache_key), cache_key);
    let _ = std::fs::write(&manifest_path, serde_json::to_string(&manifest).unwrap_or_default());

    // Return the data URI for immediate use
    let b64 = base64::engine::general_purpose::STANDARD.encode(&png_bytes);
    Ok(format!("data:image/png;base64,{}", b64))
}

#[tauri::command]
pub async fn remove_custom_icon(app: AppHandle, path: String, name: Option<String>) -> Result<(), String> {
    let cache_key = get_cache_key(&path, name.as_deref());
    let icons_dir = get_custom_icons_dir(&app)?;
    let sanitized = sanitize_filename(&cache_key);
    let filename = format!("{}.png", sanitized);
    let file_path = icons_dir.join(&filename);
    if file_path.exists() {
        std::fs::remove_file(&file_path).map_err(|e| e.to_string())?;
    }
    // Remove from manifest
    let manifest_path = icons_dir.join("custom_icons.json");
    if let Ok(content) = std::fs::read_to_string(&manifest_path) {
        if let Ok(mut manifest) = serde_json::from_str::<std::collections::HashMap<String, String>>(&content) {
            manifest.remove(&sanitized);
            let _ = std::fs::write(&manifest_path, serde_json::to_string(&manifest).unwrap_or_default());
        }
    }
    // Also remove from persistent cache so the original icon is re-fetched
    if let Some(c) = ICON_CACHE.get() {
        if let Ok(mut lock) = c.lock() {
            lock.remove(&cache_key);
        }
    }
    schedule_icon_cache_save(app);
    Ok(())
}

#[tauri::command]
pub async fn get_custom_icons(app: AppHandle) -> Result<HashMap<String, String>, String> {
    let icons_dir = get_custom_icons_dir(&app)?;
    let manifest_path = icons_dir.join("custom_icons.json");
    let manifest: std::collections::HashMap<String, String> = if let Ok(content) = std::fs::read_to_string(&manifest_path) {
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        std::collections::HashMap::new()
    };
    let mut result = HashMap::new();
    if let Ok(entries) = std::fs::read_dir(&icons_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("png") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    // Use original cache key from manifest, fallback to sanitized stem
                    let key = manifest.get(stem).cloned().unwrap_or_else(|| stem.to_string());
                    if let Ok(data) = std::fs::read(&path) {
                        use base64::Engine;
                        let b64 = base64::engine::general_purpose::STANDARD.encode(&data);
                        result.insert(key, format!("data:image/png;base64,{}", b64));
                    }
                }
            }
        }
    }
    Ok(result)
}

#[tauri::command]
pub async fn get_installed_apps() -> Vec<AppInfo> {
    let cache = INSTALLED_APPS_CACHE.get_or_init(|| std::sync::Mutex::new(Vec::new()));
    let is_empty = if let Ok(lock) = cache.lock() { lock.is_empty() } else { true };
    if is_empty && !IS_SCANNING.load(Ordering::Relaxed) { crate::services::trigger_app_scan(); }
    let start = std::time::Instant::now();
    while IS_SCANNING.load(Ordering::Relaxed) && start.elapsed() < std::time::Duration::from_secs(5) { tokio::time::sleep(std::time::Duration::from_millis(100)).await; }
    if let Ok(cache_lock) = cache.lock() { cache_lock.clone() } else { Vec::new() }
}

#[tauri::command]
pub fn hide_native_osd() {
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::{FindWindowA, ShowWindow, SW_HIDE};
        let class1 = windows::core::PCSTR(c"NativeHWNDHost".as_ptr() as *const u8);
        if let Ok(hwnd) = FindWindowA(class1, windows::core::PCSTR::null()) { let _ = ShowWindow(hwnd, SW_HIDE); }
    }
}

fn bring_window_to_front(app: &AppHandle, label: &str) {
    if let Some(win) = app.get_webview_window(label) {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
        if let Ok(hwnd) = win.hwnd() {
            unsafe {
                use windows::Win32::UI::WindowsAndMessaging::{SetForegroundWindow, ShowWindow, SW_RESTORE, SW_SHOW};
                let _ = ShowWindow(hwnd, SW_SHOW);
                let _ = ShowWindow(hwnd, SW_RESTORE);
                let _ = SetForegroundWindow(hwnd);
            }
        }
    }
}

#[tauri::command]
pub fn open_settings_window(app: AppHandle) {
    bring_window_to_front(&app, "settings");
}

#[tauri::command]
pub fn open_calendar_window(app: AppHandle) {
    bring_window_to_front(&app, "calendar");
}

#[tauri::command]
pub fn open_tray_window(app: AppHandle) {
    let anchor = notch_anchor(&app);
    std::thread::spawn(move || crate::tray_icons::open_native_tray(anchor));
}

fn notch_anchor(app: &AppHandle) -> Option<(i32, i32)> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
    let mut cursor = POINT::default();
    unsafe { GetCursorPos(&mut cursor).ok()?; }

    let windows = crate::state::main_window_rects().lock().ok()?.clone();
    let label = windows.iter()
        .find(|(_, (pos, size))| {
            cursor.x >= pos.x && cursor.x <= pos.x + size.width as i32 && cursor.y >= pos.y && cursor.y <= pos.y + size.height as i32
        })
        .map(|(label, _)| label.clone())
        .unwrap_or_else(|| "main".to_string());
    let (pos, _) = windows.get(&label).copied()?;
    let scale = app.get_webview_window(&label)?.scale_factor().ok()?;
    let rect = crate::state::notch_rects().lock().ok()?.get(&label).copied()?;

    let centre_x = pos.x + ((rect.x as f64 + rect.width as f64 / 2.0) * scale) as i32;
    let bottom = pos.y + ((rect.y as f64 + rect.height as f64) * scale) as i32;
    Some((centre_x, bottom))
}

#[tauri::command]
pub fn hide_overlay(app: AppHandle) {
    if let Some(win) = app.get_webview_window("overlay") { let _ = win.hide(); }
}

#[tauri::command]
pub fn set_splash_fullscreen(app: AppHandle, fullscreen: bool) {
    let already_fullscreen = crate::state::OVERLAY_IN_SPLASH.swap(fullscreen, Ordering::Relaxed);
    if let Some(win) = app.get_webview_window("overlay") {
        if fullscreen {
            if already_fullscreen && win.is_visible().unwrap_or(false) {
                return;
            }
            let _ = win.hide();
            if let Ok(Some(monitor)) = win.primary_monitor() {
                let size = monitor.size();
                let pos = monitor.position();
                let _ = win.set_position(tauri::PhysicalPosition::new(pos.x, pos.y));
                let _ = win.set_size(tauri::PhysicalSize::new(size.width, size.height));
            }
            let _ = win.show();
        } else {
            let _ = win.hide();
            crate::services::sync_overlays(&app);
        }
    }
}

#[tauri::command]
pub fn sync_overlay_position(app: AppHandle) {
    crate::services::sync_overlays(&app);
}

#[tauri::command]
pub fn open_wifi_settings() {
    unsafe {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
        use windows::Win32::UI::Shell::ShellExecuteA;
        let _ = ShellExecuteA(Some(HWND(std::ptr::null_mut())), windows::core::PCSTR(c"open".as_ptr() as *const u8), windows::core::PCSTR(c"ms-availablenetworks:".as_ptr() as *const u8), windows::core::PCSTR::null(), windows::core::PCSTR::null(), SW_SHOWNORMAL);
    }
}

#[tauri::command]
pub fn open_sound_settings() {
    unsafe {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
        use windows::Win32::UI::Shell::ShellExecuteA;
        let _ = ShellExecuteA(Some(HWND(std::ptr::null_mut())), windows::core::PCSTR(c"open".as_ptr() as *const u8), windows::core::PCSTR(c"ms-settings:sound".as_ptr() as *const u8), windows::core::PCSTR::null(), windows::core::PCSTR::null(), SW_SHOWNORMAL);
    }
}

#[tauri::command]
pub fn open_notification_center() {
    unsafe {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
        use windows::Win32::UI::Shell::ShellExecuteA;
        let _ = ShellExecuteA(Some(HWND(std::ptr::null_mut())), windows::core::PCSTR(c"open".as_ptr() as *const u8), windows::core::PCSTR(c"ms-actioncenter:".as_ptr() as *const u8), windows::core::PCSTR::null(), windows::core::PCSTR::null(), SW_SHOWNORMAL);
    }
}

#[tauri::command]
pub fn media_play_pause() { if let Some(sender) = COMMAND_SENDER.get() { let _ = sender.send(crate::types::SystemCommand::MediaPlayPause); } }

#[tauri::command]
pub fn media_next() { if let Some(sender) = COMMAND_SENDER.get() { let _ = sender.send(crate::types::SystemCommand::MediaNext); } }

#[tauri::command]
pub fn media_previous() { if let Some(sender) = COMMAND_SENDER.get() { let _ = sender.send(crate::types::SystemCommand::MediaPrevious); } }

#[tauri::command]
pub fn media_seek(position_ms: f64) { if let Some(sender) = COMMAND_SENDER.get() { let _ = sender.send(crate::types::SystemCommand::MediaSeek(position_ms as i64)); } }

#[tauri::command]
pub fn open_media_source_app() {
    unsafe {
        use windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager;
        use windows::Win32::UI::WindowsAndMessaging::{SetForegroundWindow, ShowWindow, SW_RESTORE, IsIconic, IsWindowVisible, FindWindowW, EnumWindows, GetWindowThreadProcessId};
        use windows::Win32::Foundation::{HWND, LPARAM};
        use windows::Win32::System::Threading::OpenProcess;

        let mgr = match GlobalSystemMediaTransportControlsSessionManager::RequestAsync().and_then(|op| op.get()) {
            Ok(m) => m,
            Err(_) => return,
        };
        let session = match mgr.GetCurrentSession() {
            Ok(s) => s,
            Err(_) => return,
        };
        let app_id = match session.SourceAppUserModelId() {
            Ok(id) => id.to_string(),
            Err(_) => return,
        };
        if app_id.is_empty() { return; }

        // First try: FindWindow with the AppUserModelId directly (works for UWP)
        let h_app_id = windows::core::HSTRING::from(&app_id);
        if let Ok(hwnd) = FindWindowW(None, windows::core::PCWSTR(h_app_id.as_ptr())) {
            if !hwnd.is_invalid() {
                if !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() {
                    let _ = ShowWindow(hwnd, SW_RESTORE);
                }
                let _ = SetForegroundWindow(hwnd);
                return;
            }
        }

        // Second try: find window by process name extracted from app_id
        let process_name = app_id.split('.').next().unwrap_or(&app_id).to_lowercase();

        struct EnumData { process_name: String, found_hwnd: Option<HWND> }

        unsafe extern "system" fn enum_callback(hwnd: HWND, lparam: LPARAM) -> windows::core::BOOL {
            let data = &mut *(lparam.0 as *mut EnumData);
            if !IsWindowVisible(hwnd).as_bool() {
                return windows::core::BOOL(1);
            }
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if pid == 0 { return windows::core::BOOL(1); }

            if let Ok(proc) = OpenProcess(
                windows::Win32::System::Threading::PROCESS_QUERY_LIMITED_INFORMATION,
                false,
                pid,
            ) {
                let mut buf = [0u16; 260];
                let mut size = buf.len() as u32;
                let ok = windows::Win32::System::Threading::QueryFullProcessImageNameW(
                    proc,
                    windows::Win32::System::Threading::PROCESS_NAME_FORMAT(0),
                    windows::core::PWSTR(buf.as_mut_ptr()),
                    &mut size,
                );
                let _ = windows::Win32::Foundation::CloseHandle(proc);
                if ok.is_ok() {
                    let path = String::from_utf16_lossy(&buf[..size as usize]);
                    let file_name = path.rsplit('\\').next().unwrap_or("").to_lowercase().replace(".exe", "");
                    if file_name == data.process_name {
                        data.found_hwnd = Some(hwnd);
                        return windows::core::BOOL(0);
                    }
                }
            }
            windows::core::BOOL(1)
        }

        let mut data = EnumData { process_name, found_hwnd: None };
        let callback: unsafe extern "system" fn(HWND, LPARAM) -> windows::core::BOOL = enum_callback;
        let _ = EnumWindows(Some(callback), LPARAM(&mut data as *mut EnumData as isize));

        if let Some(hwnd) = data.found_hwnd {
            if !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            }
            let _ = SetForegroundWindow(hwnd);
        }
    }
}

#[tauri::command]
pub fn set_volume(volume: f32) { if let Some(sender) = COMMAND_SENDER.get() { let _ = sender.send(crate::types::SystemCommand::SetVolume(volume)); } }

/// Full path of a process, None if we can't open it.
unsafe fn process_image_path(pid: u32) -> Option<String> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };

    let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
    let mut buf = [0u16; 1024];
    let mut size = buf.len() as u32;
    let res = QueryFullProcessImageNameW(
        handle,
        PROCESS_NAME_WIN32,
        windows::core::PWSTR(buf.as_mut_ptr()),
        &mut size,
    );
    let _ = CloseHandle(handle);
    res.ok()?;
    Some(String::from_utf16_lossy(&buf[..size as usize]))
}

/// Friendly exe name from version info (FileDescription), else the file stem.
/// Cached because the mixer polls.
unsafe fn friendly_process_name(path: &str) -> String {
    let cache = PROCESS_NAME_CACHE.get_or_init(|| std::sync::Mutex::new(HashMap::new()));
    if let Ok(guard) = cache.lock() {
        if let Some(name) = guard.get(path) {
            return name.clone();
        }
    }
    let name = friendly_process_name_uncached(path);
    if let Ok(mut guard) = cache.lock() {
        guard.insert(path.to_string(), name.clone());
    }
    name
}

unsafe fn friendly_process_name_uncached(path: &str) -> String {
    use windows::Win32::Storage::FileSystem::{
        GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW,
    };

    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    let filename = windows::core::PCWSTR(wide.as_ptr());

    'description: {
        let size = GetFileVersionInfoSizeW(filename, None);
        if size == 0 {
            break 'description;
        }
        let mut data = vec![0u8; size as usize];
        if GetFileVersionInfoW(filename, None, size, data.as_mut_ptr() as *mut _).is_err() {
            break 'description;
        }

        // FileDescription is under the first language/codepage pair in the translation table.
        let mut trans_ptr: *mut core::ffi::c_void = std::ptr::null_mut();
        let mut trans_len = 0u32;
        if !VerQueryValueW(
            data.as_ptr() as *const _,
            windows::core::w!("\\VarFileInfo\\Translation"),
            &mut trans_ptr,
            &mut trans_len,
        )
        .as_bool()
            || trans_len < 4
            || trans_ptr.is_null()
        {
            break 'description;
        }
        // Need all four bytes of the (LANGID, codepage) pair, read bytewise to avoid alignment issues.
        let translation = std::slice::from_raw_parts(trans_ptr as *const u8, 4);
        let lang = u16::from_le_bytes([translation[0], translation[1]]);
        let codepage = u16::from_le_bytes([translation[2], translation[3]]);

        let query = format!("\\StringFileInfo\\{lang:04x}{codepage:04x}\\FileDescription");
        let query_wide: Vec<u16> = query.encode_utf16().chain(std::iter::once(0)).collect();
        let mut value_ptr: *mut core::ffi::c_void = std::ptr::null_mut();
        let mut value_len = 0u32;
        if !VerQueryValueW(
            data.as_ptr() as *const _,
            windows::core::PCWSTR(query_wide.as_ptr()),
            &mut value_ptr,
            &mut value_len,
        )
        .as_bool()
            || value_len == 0
            || value_ptr.is_null()
        {
            break 'description;
        }

        let chars = std::slice::from_raw_parts(value_ptr as *const u16, value_len as usize);
        let description = String::from_utf16_lossy(chars)
            .trim_end_matches('\0')
            .trim()
            .to_string();
        if !description.is_empty() {
            return description;
        }
    }

    std::path::Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|stem| {
            let mut chars = stem.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => stem.to_string(),
            }
        })
        .unwrap_or_else(|| "Unknown app".to_string())
}

/// Runs f for every audio session on every active output (sets up and tears down COM itself).
unsafe fn for_each_audio_session<F: FnMut(&windows::Win32::Media::Audio::IAudioSessionControl2)>(
    mut f: F,
) -> Result<(), String> {
    use windows::Win32::Media::Audio::{
        eRender, IAudioSessionControl2, IAudioSessionManager2, IMMDeviceEnumerator,
        DEVICE_STATE_ACTIVE,
    };
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED,
    };

    let com_initialized = CoInitializeEx(None, COINIT_MULTITHREADED).is_ok();
    let result = (|| -> Result<(), String> {
        let enumerator: IMMDeviceEnumerator = CoCreateInstance(
            &windows::Win32::Media::Audio::MMDeviceEnumerator,
            None,
            CLSCTX_ALL,
        )
        .map_err(|e| e.to_string())?;
        // Per-app output routing can put an app on a non-default device, so scan all active outputs.
        let devices = enumerator
            .EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)
            .map_err(|e| e.to_string())?;
        let device_count = devices.GetCount().map_err(|e| e.to_string())?;
        for i in 0..device_count {
            let Ok(device) = devices.Item(i) else {
                continue;
            };
            let Ok(manager) = device.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None) else {
                continue;
            };
            let Ok(sessions) = manager.GetSessionEnumerator() else {
                continue;
            };
            let Ok(count) = sessions.GetCount() else {
                continue;
            };
            for j in 0..count {
                let Ok(control) = sessions.GetSession(j) else {
                    continue;
                };
                let Ok(control) = control.cast::<IAudioSessionControl2>() else {
                    continue;
                };
                f(&control);
            }
        }
        Ok(())
    })();
    if com_initialized {
        CoUninitialize();
    }
    result
}

/// Apps with an audio session, one per process, playing ones first.
#[tauri::command]
pub async fn get_audio_sessions() -> Result<Vec<AudioSessionInfo>, String> {
    tauri::async_runtime::spawn_blocking(|| unsafe {
        use std::collections::HashSet;
        use windows::Win32::Media::Audio::{
            AudioSessionStateActive, AudioSessionStateExpired, ISimpleAudioVolume,
        };

        let own_pid = std::process::id();
        let mut seen: HashSet<u32> = HashSet::new();
        let mut found: Vec<(AudioSessionInfo, bool)> = Vec::new();

        for_each_audio_session(|control| {
            // Only S_OK is the system sounds session.
            if control.IsSystemSoundsSession() == windows::Win32::Foundation::S_OK {
                return;
            }
            let Ok(pid) = control.GetProcessId() else {
                return;
            };
            if pid == 0 || pid == own_pid || !seen.insert(pid) {
                return;
            }
            let state = control.GetState().unwrap_or_default();
            if state == AudioSessionStateExpired {
                seen.remove(&pid);
                return;
            }
            let Ok(volume_ctl) = control.cast::<ISimpleAudioVolume>() else {
                return;
            };
            let process_path = process_image_path(pid);
            let name = match process_path.as_deref() {
                Some(path) => friendly_process_name(path),
                None => format!("Process {pid}"),
            };
            found.push((
                AudioSessionInfo {
                    pid,
                    name,
                    process_path,
                    volume: volume_ctl.GetMasterVolume().unwrap_or(1.0),
                    is_muted: volume_ctl.GetMute().map(|m| m.as_bool()).unwrap_or(false),
                },
                state == AudioSessionStateActive,
            ));
        })?;

        found.sort_by(|a, b| {
            b.1.cmp(&a.1)
                .then_with(|| a.0.name.to_lowercase().cmp(&b.0.name.to_lowercase()))
        });
        Ok(found.into_iter().map(|(info, _)| info).collect())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn set_app_volume(pid: u32, volume: f32) -> Result<(), String> {
    let volume = volume.clamp(0.0, 1.0);
    tauri::async_runtime::spawn_blocking(move || unsafe {
        use windows::Win32::Media::Audio::ISimpleAudioVolume;

        for_each_audio_session(|control| {
            if control.GetProcessId().ok() != Some(pid) {
                return;
            }
            if let Ok(volume_ctl) = control.cast::<ISimpleAudioVolume>() {
                let _ = volume_ctl.SetMasterVolume(volume, std::ptr::null());
                if volume > 0.0 {
                    let _ = volume_ctl.SetMute(false, std::ptr::null());
                }
            }
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn set_app_mute(pid: u32, muted: bool) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || unsafe {
        use windows::Win32::Media::Audio::ISimpleAudioVolume;

        for_each_audio_session(|control| {
            if control.GetProcessId().ok() != Some(pid) {
                return;
            }
            if let Ok(volume_ctl) = control.cast::<ISimpleAudioVolume>() {
                let _ = volume_ctl.SetMute(muted, std::ptr::null());
            }
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Mixer panel bounds in overlay CSS px, so the mouse hook keeps the card clickable while open.
#[tauri::command]
pub fn set_volume_mixer_rect(x: f64, y: f64, width: f64, height: f64) {
    if let Ok(mut rect) = VOLUME_MIXER_RECT.lock() {
        *rect = Some((x, y, width, height));
    }
}

#[tauri::command]
pub fn clear_volume_mixer_rect() {
    if let Ok(mut rect) = VOLUME_MIXER_RECT.lock() {
        *rect = None;
    }
}

/// Shared by the tray menu, the in-app Quit button, and the window CloseRequested
/// handlers so that any shutdown path (including Task Manager's WM_CLOSE) behaves
/// identically.
pub fn restore_taskbar_and_exit(handle: &AppHandle) {
    SHUTTING_DOWN.store(true, Ordering::Relaxed);
    NATIVE_TASKBAR_HIDDEN.store(false, Ordering::SeqCst);
    for (_, w) in handle.webview_windows() {
        let _ = w.hide();
    }
    for (label, w) in handle.webview_windows() {
        let registered = if crate::state::is_notch_label(&label) {
            crate::state::get_flag(crate::state::main_appbar_registered(), &label)
        } else if crate::state::is_dock_label(&label) {
            crate::state::get_flag(crate::state::dock_appbar_registered(), &label)
        } else {
            false
        };
        if registered {
            if let Ok(hwnd) = w.hwnd() { unregister_appbar_native(hwnd); }
        }
    }
    set_taskbar_visibility(true, true);
    NATIVE_TASKBAR_HIDDEN.store(false, Ordering::Relaxed);
    // Destroy all webview windows before exiting so Chromium's UnregisterClass for
    // Chrome_WidgetWin_0 finds no open windows (avoids the harmless Error=1412 log).
    for (_, w) in handle.webview_windows() {
        let _ = w.destroy();
    }
    handle.exit(0);
}

#[tauri::command]
pub async fn quit_nectar(handle: AppHandle) {
    restore_taskbar_and_exit(&handle);
}

fn registered_uninstaller() -> Option<std::path::PathBuf> {
    use std::os::windows::process::CommandExt;
    for root in ["HKCU", "HKLM"] {
        let key = format!(r"{}\Software\Microsoft\Windows\CurrentVersion\Uninstall\Nectar", root);
        let Ok(out) = std::process::Command::new("reg")
            .args(["query", &key, "/v", "UninstallString"])
            .creation_flags(0x0800_0000)
            .output() else { continue };
        let text = String::from_utf8_lossy(&out.stdout);
        let Some(line) = text.lines().find(|l| l.contains("REG_SZ")) else { continue };
        let value = line.split("REG_SZ").nth(1).unwrap_or("").trim();
        let exe = if let Some(rest) = value.strip_prefix('"') {
            rest.split('"').next().unwrap_or("")
        } else {
            value.split(" --").next().unwrap_or("")
        };
        let path = std::path::PathBuf::from(exe);
        if !exe.is_empty() && path.exists() { return Some(path); }
    }
    None
}

#[tauri::command]
pub async fn uninstall_nectar(handle: AppHandle) -> Result<(), String> {
    crate::uninstall_registry::heal_uninstall_registration();

    let mut uninstaller = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .parent()
        .ok_or_else(|| "Could not resolve install directory".to_string())?
        .join("uninstall.exe");

    if !uninstaller.exists() {
        uninstaller = registered_uninstaller().ok_or_else(|| {
            "This copy of Nectar isn't an installed one (it's a portable or development build), so there's nothing to uninstall. Run it from its install folder, or use Windows Settings > Apps.".to_string()
        })?;
    }

    std::process::Command::new(&uninstaller)
        .arg("--uninstall")
        .current_dir(std::env::temp_dir())
        .spawn()
        .map_err(|e| format!("Couldn't launch {}: {}", uninstaller.display(), e))?;

    restore_taskbar_and_exit(&handle);
    Ok(())
}

#[tauri::command]
pub async fn get_diagnostics(app: AppHandle) -> String {
    tauri::async_runtime::spawn_blocking(move || crate::diagnostics::build_report(&app))
        .await
        .unwrap_or_else(|e| format!("Couldn't build the report: {e}"))
}

#[tauri::command]
pub fn open_log_folder() -> Result<(), String> {
    let dir = crate::diagnostics::log_dir().ok_or_else(|| "No log folder yet".to_string())?;
    std::process::Command::new("explorer")
        .arg(&dir)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_hitbox_logging(enabled: bool) {
    crate::diagnostics::set_hitbox_logging(enabled);
}

#[tauri::command]
pub fn get_hitbox_logging() -> bool {
    crate::diagnostics::hitbox_logging()
}

#[tauri::command]
pub async fn restart_nectar(handle: AppHandle) {
    #[cfg(debug_assertions)]
    {
        // Dev just reloads the pages. Leave SHUTTING_DOWN alone here or the hooks stay off.
        for (_, w) in handle.webview_windows() {
            let _ = w.eval("window.location.reload()");
        }
        return;
    }
    #[cfg(not(debug_assertions))]
    {
        relaunch_nectar(&handle);
    }
}

/// Relaunches the process. Also used for webview crashes, reloading pages can't fix those.
pub fn relaunch_nectar(handle: &AppHandle) {
    SHUTTING_DOWN.store(true, Ordering::Relaxed);
    if let Some(w) = handle.get_webview_window("main") { if let Ok(hwnd) = w.hwnd() { unregister_appbar_native(hwnd); } }
    if let Some(w) = handle.get_webview_window("dock") { if let Ok(hwnd) = w.hwnd() { unregister_appbar_native(hwnd); } }
    if let Some(w) = handle.get_webview_window("settings") { let _ = w.destroy(); }
    std::env::set_var("NECTAR_RESTARTING", "1");
    close_single_instance_handles();
    handle.restart();
}

fn terminate_process(pid: u32) -> Result<(), bool> {
    use windows::Win32::Foundation::{CloseHandle, ERROR_ACCESS_DENIED};
    use windows::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};
    match unsafe { OpenProcess(PROCESS_TERMINATE, false, pid) } {
        Ok(handle) => {
            let terminated = unsafe { TerminateProcess(handle, 1).is_ok() };
            unsafe { let _ = CloseHandle(handle); }
            if terminated { Ok(()) } else { Err(false) }
        }
        Err(e) => Err(e.code() == windows::core::HRESULT::from_win32(ERROR_ACCESS_DENIED.0)),
    }
}

fn kill_elevated(pid: u32) -> bool {
    use windows::core::PCWSTR;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;
    let verb = crate::uninstall_registry::wide("runas");
    let file = crate::uninstall_registry::wide("taskkill");
    let args = crate::uninstall_registry::wide(&format!("/F /T /PID {pid}"));
    let result = unsafe {
        ShellExecuteW(None, PCWSTR(verb.as_ptr()), PCWSTR(file.as_ptr()), PCWSTR(args.as_ptr()), None, SW_HIDE)
    };
    (result.0 as isize) > 32
}

#[tauri::command]
pub async fn end_task(hwnd: isize, all_hwnds: Option<Vec<isize>>) {
    use std::os::windows::process::CommandExt;
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumChildWindows, GetWindowThreadProcessId, PostMessageW, WM_CLOSE,
    };
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    struct HostScan { host_pid: u32, found: u32 }
    unsafe extern "system" fn child_proc(child: HWND, lparam: LPARAM) -> windows::core::BOOL {
        let scan = &mut *(lparam.0 as *mut HostScan);
        let mut pid = 0u32;
        GetWindowThreadProcessId(child, Some(&mut pid));
        if pid != 0 && pid != scan.host_pid {
            scan.found = pid;
            return windows::core::BOOL(0);
        }
        windows::core::BOOL(1)
    }

    unsafe fn image_name(pid: u32) -> Option<String> {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 520];
        let mut len = buf.len() as u32;
        let res = QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, windows::core::PWSTR(buf.as_mut_ptr()), &mut len);
        let _ = windows::Win32::Foundation::CloseHandle(handle);
        res.ok()?;
        Some(String::from_utf16_lossy(&buf[..len as usize]))
    }

    unsafe fn app_pid(hwnd: HWND) -> Option<u32> {
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 { return None; }
        let name = image_name(pid)?.to_lowercase();
        if name.ends_with("\\explorer.exe") { return None; }
        if name.ends_with("\\applicationframehost.exe") {
            let mut scan = HostScan { host_pid: pid, found: 0 };
            let _ = EnumChildWindows(Some(hwnd), Some(child_proc), LPARAM(&mut scan as *mut HostScan as isize));
            return if scan.found != 0 { Some(scan.found) } else { None };
        }
        Some(pid)
    }

    tauri::async_runtime::spawn_blocking(move || unsafe {
        let mut handles = vec![hwnd];
        handles.extend(all_hwnds.unwrap_or_default());

        let own_pid = std::process::id();
        let mut by_pid: std::collections::BTreeMap<u32, Vec<isize>> = std::collections::BTreeMap::new();
        let mut politely = Vec::new();
        for h in handles {
            match app_pid(HWND(h as *mut _)) {
                Some(pid) if pid != own_pid => by_pid.entry(pid).or_default().push(h),
                Some(_) => {}
                None => politely.push(h),
            }
        }

        for (pid, hwnds) in by_pid {
            let killed = std::process::Command::new("taskkill")
                .args(["/F", "/T", "/PID", &pid.to_string()])
                .creation_flags(0x0800_0000)
                .output()
                .is_ok_and(|o| o.status.success());
            if killed {
                crate::diagnostics::log(&format!("end task: taskkill ended pid {pid}"));
                continue;
            }
            match terminate_process(pid) {
                Ok(()) => {
                    crate::diagnostics::log(&format!("end task: terminated pid {pid} directly"));
                    continue;
                }
                Err(denied) => {
                    crate::diagnostics::log(&format!("end task: could not terminate pid {pid}, access denied: {denied}"));
                    if denied {
                        let asked = kill_elevated(pid);
                        crate::diagnostics::log(&format!("end task: elevation prompt for pid {pid} started: {asked}"));
                    }
                }
            }
            politely.extend(hwnds);
        }
        for h in politely {
            let _ = PostMessageW(Some(HWND(h as *mut _)), WM_CLOSE, windows::Win32::Foundation::WPARAM(0), windows::Win32::Foundation::LPARAM(0));
        }
    }).await.unwrap_or_default();
}

#[tauri::command]
pub async fn close_window(hwnd: isize) {
    tauri::async_runtime::spawn_blocking(move || unsafe {
        use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_CLOSE};
        let hwnd = HWND(hwnd as *mut _);
        let _ = PostMessageW(Some(hwnd), WM_CLOSE, windows::Win32::Foundation::WPARAM(0), windows::Win32::Foundation::LPARAM(0));
    }).await.unwrap_or_default();
}

fn re_register_appbars(app: &AppHandle, settings: &HashMap<String, serde_json::Value>) {
    let notch_fixed = settings.get("nectar-notch-mode").map(|v| v.as_str() == Some("fixed")).unwrap_or(true);
    let dock_fixed = settings.get("nectar-dock-mode").map(|v| v.as_str() == Some("fixed")).unwrap_or(false);
    for (label, win) in app.webview_windows() {
        if crate::state::is_notch_label(&label) && notch_fixed {
            crate::services::register_appbar(win);
        } else if crate::state::is_dock_label(&label) && dock_fixed {
            crate::services::register_dock_appbar(win);
        }
    }
}

const MONITOR_TARGETING_KEYS: [&str; 5] = [
    "nectar-dock-monitor-mode",
    "nectar-dock-monitor-id",
    "nectar-notch-monitor-mode",
    "nectar-notch-monitor-id",
    "nectar-dock-enabled",
];

#[tauri::command]
pub fn save_setting(app: AppHandle, key: String, value: serde_json::Value) -> Result<(), String> {
    let path = app.path().app_config_dir().map_err(|e| e.to_string())?.join("settings.json");
    if let Some(parent) = path.parent() { let _ = std::fs::create_dir_all(parent); }
    let mut settings = HashMap::new();
    if let Ok(content) = std::fs::read_to_string(&path) { if let Ok(existing) = serde_json::from_str::<HashMap<String, serde_json::Value>>(&content) { settings = existing; } }
    settings.insert(key.clone(), value);
    let content = serde_json::to_string(&settings).map_err(|e| e.to_string())?;
    std::fs::write(path, content).map_err(|e| e.to_string())?;

    crate::utils::replace_settings_cache(settings.clone());

    let _ = app.emit("settings-changed", serde_json::json!({ "key": &key, "value": &settings[&key] }));

    if key == "nectar-scale" {
        re_register_appbars(&app, &settings);
    }

    if MONITOR_TARGETING_KEYS.contains(&key.as_str()) {
        let app_clone = app.clone();
        tauri::async_runtime::spawn(async move {
            crate::monitors::sync_monitor_windows(&app_clone);
            let dock_enabled = crate::utils::get_setting_str(&app_clone, "nectar-dock-enabled")
                .map(|v| v == "true").unwrap_or(true);
            if dock_enabled {
                let dock_mode = crate::utils::get_setting_str(&app_clone, "nectar-dock-mode")
                    .unwrap_or_else(|| "smart".to_string());
                change_dock_mode(app_clone.clone(), dock_mode).await;
            }
            let notch_mode = crate::utils::get_setting_str(&app_clone, "nectar-notch-mode")
                .unwrap_or_else(|| "fixed".to_string());
            change_notch_mode(app_clone.clone(), notch_mode).await;
        });
    }

    Ok(())
}

#[tauri::command]
pub fn get_monitors(app: AppHandle) -> Vec<crate::types::MonitorInfo> {
    crate::monitors::list_monitors(&app)
}

#[tauri::command]
pub fn get_dock_mode_for_window(window: tauri::WebviewWindow, app: AppHandle) -> String {
    crate::monitors::resolve_mode_for_label(&app, crate::types::WindowKind::Dock, window.label())
}

#[tauri::command]
pub fn get_notch_mode_for_window(window: tauri::WebviewWindow, app: AppHandle) -> String {
    crate::monitors::resolve_mode_for_label(&app, crate::types::WindowKind::Notch, window.label())
}

#[tauri::command]
pub fn set_dock_mode_for_window(window: tauri::WebviewWindow, app: AppHandle, mode: String) -> Result<(), String> {
    crate::monitors::set_mode_for_label(&app, crate::types::WindowKind::Dock, window.label(), &mode)
}

#[tauri::command]
pub fn set_notch_mode_for_window(window: tauri::WebviewWindow, app: AppHandle, mode: String) -> Result<(), String> {
    crate::monitors::set_mode_for_label(&app, crate::types::WindowKind::Notch, window.label(), &mode)
}


#[tauri::command]
pub fn load_settings(app: AppHandle) -> Result<HashMap<String, serde_json::Value>, String> {
    let path = match app.path().app_config_dir() {
        Ok(p) => p.join("settings.json"),
        Err(_) => return Ok(HashMap::new()),
    };
    if let Ok(content) = std::fs::read_to_string(path) { if let Ok(settings) = serde_json::from_str(&content) { return Ok(settings); } }
    Ok(HashMap::new())
}

#[tauri::command]
pub fn reset_settings(app: AppHandle) -> Result<(), String> {
    let path = app.path().app_config_dir().map_err(|e| e.to_string())?.join("settings.json");
    std::fs::write(&path, "{}").map_err(|e| e.to_string())?;
    crate::utils::replace_settings_cache(HashMap::new());
    Ok(())
}

#[tauri::command]
pub async fn capture_window_thumbnail(hwnd: isize, max_width: u32, max_height: u32) -> Result<Option<(String, i64)>, String> {
    tauri::async_runtime::spawn_blocking(move || unsafe {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{IsWindow, IsIconic};

        let hwnd = HWND(hwnd as *mut _);
        if !IsWindow(Some(hwnd)).as_bool() { return None; }

        let hwnd_key = hwnd.0 as isize;
        let is_minimized = IsIconic(hwnd).as_bool();

        let mut focus_time = 0i64;
        if let Some(map) = crate::state::FOCUS_TIMESTAMPS.get() {
            if let Ok(guard) = map.lock() {
                focus_time = guard.get(&hwnd_key).copied().unwrap_or(0);
            }
        }

        if is_minimized {
            // Don't restore a minimized window to capture it, it flashes on screen. The hooks keep
            // thumbnails warm, so without one we just show no preview.
            let cached_img = if let Some(cache) = crate::state::THUMBNAIL_CACHE.get() {
                cache.lock().ok().and_then(|g| g.get(&hwnd_key).map(|(img, _)| img.clone()))
            } else {
                None
            };
            return cached_img.map(|img| (img, focus_time));
        }

        let result = crate::utils::capture_hwnd_to_base64(hwnd, max_width, max_height);

        if let Some(ref img) = result {
            if let Some(cache) = crate::state::THUMBNAIL_CACHE.get() {
                if let Ok(mut guard) = cache.lock() {
                    guard.insert(hwnd_key, (img.clone(), crate::utils::get_now_ms()));
                    // Prune dead windows if cache size exceeds 15
                    // Minimized windows (IsWindow == true) are kept intact
                    if guard.len() > 15 {
                        guard.retain(|&k, _| {
                            IsWindow(Some(HWND(k as *mut _))).as_bool()
                        });
                    }
                }
            }
        }

        result.map(|img| (img, focus_time))
    }).await.map_err(|e| e.to_string())
}


// ── Radio helpers — Windows Runtime Windows.Devices.Radios ───────────────────
// Replaces PowerShell -ExecutionPolicy Bypass scripts for WiFi/Bluetooth state.

fn get_radio_state_sync(kind: windows::Devices::Radios::RadioKind) -> Result<bool, String> {
    use windows::Devices::Radios::{Radio, RadioState};
    unsafe {
        let _ = windows::Win32::System::Com::CoInitializeEx(
            None, windows::Win32::System::Com::COINIT_MULTITHREADED,
        );
    }
    let radios = Radio::GetRadiosAsync()
        .and_then(|op| op.get())
        .map_err(|e| e.to_string())?;
    for i in 0..radios.Size().unwrap_or(0) {
        if let Ok(radio) = radios.GetAt(i) {
            if radio.Kind().ok() == Some(kind) {
                return Ok(radio.State().ok() == Some(RadioState::On));
            }
        }
    }
    Ok(false)
}

fn set_radio_state_sync(kind: windows::Devices::Radios::RadioKind, enabled: bool) -> Result<(), String> {
    use windows::Devices::Radios::{Radio, RadioState};
    unsafe {
        let _ = windows::Win32::System::Com::CoInitializeEx(
            None, windows::Win32::System::Com::COINIT_MULTITHREADED,
        );
    }
    let radios = Radio::GetRadiosAsync()
        .and_then(|op| op.get())
        .map_err(|e| e.to_string())?;
    for i in 0..radios.Size().unwrap_or(0) {
        if let Ok(radio) = radios.GetAt(i) {
            if radio.Kind().ok() == Some(kind) {
                let target = if enabled { RadioState::On } else { RadioState::Off };
                let _ = radio.SetStateAsync(target).and_then(|op| op.get());
                return Ok(());
            }
        }
    }
    Ok(())
}

#[tauri::command]
pub fn get_volume() -> f32 {
    crate::state::CURRENT_VOLUME.load(std::sync::atomic::Ordering::Relaxed) as f32 / 100.0
}

/// Live volume and mute of the default output. HUDs seed from this because the worker's
/// first volume-change fires before the overlay can listen.
#[tauri::command]
pub async fn get_volume_state() -> Result<VolumeChangeEvent, String> {
    tauri::async_runtime::spawn_blocking(|| unsafe {
        use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
        use windows::Win32::Media::Audio::{eConsole, eRender, IMMDeviceEnumerator};
        use windows::Win32::System::Com::{
            CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED,
        };

        let com_initialized = CoInitializeEx(None, COINIT_MULTITHREADED).is_ok();
        let result = (|| -> Result<VolumeChangeEvent, String> {
            let enumerator: IMMDeviceEnumerator = CoCreateInstance(
                &windows::Win32::Media::Audio::MMDeviceEnumerator,
                None,
                CLSCTX_ALL,
            )
            .map_err(|e| e.to_string())?;
            let device = enumerator
                .GetDefaultAudioEndpoint(eRender, eConsole)
                .map_err(|e| e.to_string())?;
            let endpoint: IAudioEndpointVolume = device
                .Activate(CLSCTX_ALL, None)
                .map_err(|e| e.to_string())?;
            let volume = endpoint
                .GetMasterVolumeLevelScalar()
                .map_err(|e| e.to_string())?;
            let is_muted = endpoint.GetMute().map(|m| m.as_bool()).unwrap_or(false);
            CURRENT_VOLUME.store((volume * 100.0) as u32, Ordering::Relaxed);
            Ok(VolumeChangeEvent { volume, is_muted })
        })();
        if com_initialized {
            CoUninitialize();
        }
        result
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn get_brightness() -> u32 {
    crate::state::CURRENT_BRIGHTNESS.load(std::sync::atomic::Ordering::Relaxed)
}

// Wi-Fi status: the radio state alone can't tell on from connected, so ask the WLAN API too.

fn is_wlan_connected_sync() -> bool {
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::NetworkManagement::WiFi::{
        wlan_intf_opcode_current_connection, WlanCloseHandle, WlanEnumInterfaces, WlanFreeMemory,
        WlanOpenHandle, WlanQueryInterface, WLAN_INTERFACE_INFO_LIST,
    };

    unsafe {
        let mut negotiated_version = 0u32;
        let mut client_handle = HANDLE::default();
        if WlanOpenHandle(2, None, &mut negotiated_version, &mut client_handle) != 0 {
            return false;
        }

        let mut interface_list: *mut WLAN_INTERFACE_INFO_LIST = std::ptr::null_mut();
        let mut connected = false;
        if WlanEnumInterfaces(client_handle, None, &mut interface_list) == 0
            && !interface_list.is_null()
        {
            let interfaces = std::slice::from_raw_parts(
                (*interface_list).InterfaceInfo.as_ptr(),
                (*interface_list).dwNumberOfItems as usize,
            );
            for interface in interfaces {
                let mut data_size = 0u32;
                let mut data: *mut std::ffi::c_void = std::ptr::null_mut();
                // Errors with ERROR_INVALID_STATE when not associated, so success means connected.
                let result = WlanQueryInterface(
                    client_handle,
                    &interface.InterfaceGuid,
                    wlan_intf_opcode_current_connection,
                    None,
                    &mut data_size,
                    &mut data,
                    None,
                );
                if !data.is_null() {
                    WlanFreeMemory(data);
                }
                if result == 0 {
                    connected = true;
                    break;
                }
            }
            WlanFreeMemory(interface_list as *const _);
        }

        WlanCloseHandle(client_handle, None);
        connected
    }
}

/// Wi-Fi radio and connection state for the quick settings tile.
#[tauri::command]
pub async fn get_wifi_status() -> Result<WifiStatus, String> {
    tauri::async_runtime::spawn_blocking(|| -> Result<WifiStatus, String> {
        let enabled = get_radio_state_sync(windows::Devices::Radios::RadioKind::WiFi)?;
        let connected = enabled && is_wlan_connected_sync();
        Ok(WifiStatus { enabled, connected })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn set_wifi_state(enabled: bool) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        set_radio_state_sync(windows::Devices::Radios::RadioKind::WiFi, enabled)
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn get_bluetooth_state() -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(|| {
        get_radio_state_sync(windows::Devices::Radios::RadioKind::Bluetooth)
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn set_bluetooth_state(enabled: bool) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        set_radio_state_sync(windows::Devices::Radios::RadioKind::Bluetooth, enabled)
    }).await.map_err(|e| e.to_string())?
}

// Settings openers — use ShellExecuteA directly instead of spawning powershell

#[tauri::command]
pub fn open_bluetooth_settings() {
    unsafe {
        use windows::Win32::UI::Shell::ShellExecuteA;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
        let _ = ShellExecuteA(None, windows::core::PCSTR(c"open".as_ptr() as *const u8), windows::core::PCSTR(c"ms-settings:bluetooth".as_ptr() as *const u8), windows::core::PCSTR::null(), windows::core::PCSTR::null(), SW_SHOWNORMAL);
    }
}

#[tauri::command]
pub fn open_airplane_mode_settings() {
    unsafe {
        use windows::Win32::UI::Shell::ShellExecuteA;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
        let _ = ShellExecuteA(None, windows::core::PCSTR(c"open".as_ptr() as *const u8), windows::core::PCSTR(c"ms-settings:network-airplanemode".as_ptr() as *const u8), windows::core::PCSTR::null(), windows::core::PCSTR::null(), SW_SHOWNORMAL);
    }
}

#[tauri::command]
pub fn set_brightness(app: AppHandle, brightness: u32) {
    let val = brightness.min(100);
    crate::state::CURRENT_BRIGHTNESS.store(val, Ordering::Relaxed);
    crate::state::LAST_BRIGHTNESS_CHANGE.store(crate::utils::get_now_ms(), Ordering::Relaxed);
    let _ = app.emit("brightness-change", BrightnessChangeEvent { brightness: val });
    if let Some(tx) = crate::state::BRIGHTNESS_SENDER.get() {
        let _ = tx.send(val);
    }
}

#[tauri::command]
pub async fn get_battery_saver_state() -> Result<bool, String> {
    // Uses Windows Runtime PowerManager — no PowerShell required.
    tauri::async_runtime::spawn_blocking(|| {
        unsafe {
            let _ = windows::Win32::System::Com::CoInitializeEx(
                None, windows::Win32::System::Com::COINIT_MULTITHREADED,
            );
        }
        use windows::System::Power::{EnergySaverStatus, PowerManager};
        match PowerManager::EnergySaverStatus() {
            Ok(status) => Ok(status == EnergySaverStatus::On),
            Err(_) => Ok(false), // No battery / not supported on this device
        }
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn has_battery() -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(|| {
        unsafe {
            let _ = windows::Win32::System::Com::CoInitializeEx(
                None, windows::Win32::System::Com::COINIT_MULTITHREADED,
            );
        }
        use windows::System::Power::{BatteryStatus, PowerManager};
        match PowerManager::BatteryStatus() {
            Ok(status) => Ok(status != BatteryStatus::NotPresent),
            Err(_) => Ok(true), // Unknown — default to showing it rather than hiding a real battery
        }
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn open_battery_saver_settings() {
    unsafe {
        use windows::Win32::UI::Shell::ShellExecuteA;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
        let _ = ShellExecuteA(None, windows::core::PCSTR(c"open".as_ptr() as *const u8), windows::core::PCSTR(c"ms-settings:batterysaver".as_ptr() as *const u8), windows::core::PCSTR::null(), windows::core::PCSTR::null(), SW_SHOWNORMAL);
    }
}

// --- System metrics for status widgets ---

#[tauri::command]
pub fn get_cpu_usage() -> Result<u32, String> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static LAST_IDLE: AtomicU64 = AtomicU64::new(0);
    static LAST_KERNEL: AtomicU64 = AtomicU64::new(0);
    static LAST_USER: AtomicU64 = AtomicU64::new(0);

    unsafe {
        let mut idle = windows::Win32::Foundation::FILETIME::default();
        let mut kernel = windows::Win32::Foundation::FILETIME::default();
        let mut user = windows::Win32::Foundation::FILETIME::default();
        windows::Win32::System::Threading::GetSystemTimes(Some(&mut idle), Some(&mut kernel), Some(&mut user))
            .map_err(|e| format!("GetSystemTimes failed: {e}"))?;

        let idle_u64 = (idle.dwHighDateTime as u64) << 32 | idle.dwLowDateTime as u64;
        let kernel_u64 = (kernel.dwHighDateTime as u64) << 32 | kernel.dwLowDateTime as u64;
        let user_u64 = (user.dwHighDateTime as u64) << 32 | user.dwLowDateTime as u64;

        let prev_idle = LAST_IDLE.swap(idle_u64, Ordering::Relaxed);
        let prev_kernel = LAST_KERNEL.swap(kernel_u64, Ordering::Relaxed);
        let prev_user = LAST_USER.swap(user_u64, Ordering::Relaxed);

        let idle_delta = idle_u64.saturating_sub(prev_idle);
        let kernel_delta = kernel_u64.saturating_sub(prev_kernel);
        let user_delta = user_u64.saturating_sub(prev_user);
        let total = kernel_delta + user_delta;

        if total == 0 {
            return Ok(0);
        }
        let usage = ((total - idle_delta) * 100) / total;
        Ok(usage as u32)
    }
}

#[tauri::command]
pub fn get_ram_usage() -> Result<f32, String> {
    unsafe {
        let mut mem = windows::Win32::System::SystemInformation::MEMORYSTATUSEX::default();
        mem.dwLength = std::mem::size_of::<windows::Win32::System::SystemInformation::MEMORYSTATUSEX>() as u32;
        windows::Win32::System::SystemInformation::GlobalMemoryStatusEx(&mut mem)
            .map_err(|e| format!("GlobalMemoryStatusEx failed: {e}"))?;
        Ok(mem.dwMemoryLoad as f32)
    }
}

#[tauri::command]
pub fn get_disk_space() -> Result<u64, String> {
    unsafe {
        let mut free_bytes = 0u64;
        let mut total_bytes = 0u64;
        let mut total_free = 0u64;
        windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW(
            windows::core::w!("C:\\"),
            Some(&mut free_bytes),
            Some(&mut total_bytes),
            Some(&mut total_free),
        ).map_err(|e| format!("GetDiskFreeSpaceEx failed: {e}"))?;
        Ok(free_bytes / (1024 * 1024 * 1024))
    }
}

#[derive(serde::Serialize, Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub exe_path: Option<String>,
    pub memory_kb: u64,
}

const PROTECTED_PROCESS_NAMES: &[&str] = &[
    "system",
    "system idle process",
    "registry",
    "csrss.exe",
    "wininit.exe",
    "winlogon.exe",
    "services.exe",
    "lsass.exe",
    "smss.exe",
    "explorer.exe",
    "dwm.exe",
    "nectar.exe",
];

#[tauri::command]
pub fn get_running_processes() -> Result<Vec<ProcessInfo>, String> {
    unsafe {
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::System::Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
        };
        use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
        use windows::Win32::System::Threading::{
            OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
        };

        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).map_err(|e| e.to_string())?;
        let mut entry = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        let mut results = Vec::new();

        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                let name = String::from_utf16_lossy(&entry.szExeFile)
                    .trim_end_matches('\0')
                    .to_string();
                let pid = entry.th32ProcessID;

                if pid != 0 && !name.is_empty() && !PROTECTED_PROCESS_NAMES.contains(&name.to_lowercase().as_str()) {
                    let mut exe_path = None;
                    let mut memory_kb = 0u64;
                    if let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
                        let mut buf = [0u16; 512];
                        let mut size = buf.len() as u32;
                        if QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, windows::core::PWSTR(buf.as_mut_ptr()), &mut size).is_ok() {
                            exe_path = Some(String::from_utf16_lossy(&buf[..size as usize]));
                        }
                        let mut counters = PROCESS_MEMORY_COUNTERS { cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32, ..Default::default() };
                        if GetProcessMemoryInfo(handle, &mut counters, counters.cb).is_ok() {
                            memory_kb = (counters.WorkingSetSize as u64) / 1024;
                        }
                        let _ = CloseHandle(handle);
                    }
                    results.push(ProcessInfo { pid, name, exe_path, memory_kb });
                }

                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snapshot);

        results.sort_by(|a, b| b.memory_kb.cmp(&a.memory_kb));
        Ok(results)
    }
}

#[tauri::command]
pub fn kill_process(pid: u32, name: String) -> Result<(), String> {
    if PROTECTED_PROCESS_NAMES.contains(&name.to_lowercase().as_str()) {
        return Err(format!("Refusing to end protected process: {name}"));
    }
    unsafe {
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};

        let handle = OpenProcess(PROCESS_TERMINATE, false, pid).map_err(|e| e.to_string())?;
        let result = TerminateProcess(handle, 1).map_err(|e| e.to_string());
        let _ = CloseHandle(handle);
        result
    }
}

#[tauri::command]
pub fn get_network_speed() -> Result<(u64, u64), String> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static LAST_BYTES_SENT: AtomicU64 = AtomicU64::new(0);
    static LAST_BYTES_RECV: AtomicU64 = AtomicU64::new(0);
    static LAST_CHECK: AtomicU64 = AtomicU64::new(0);

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let last_check = LAST_CHECK.load(Ordering::Relaxed);
    let elapsed = if last_check > 0 { (now - last_check).max(1) } else { 1000 };

    let (mut total_in, mut total_out) = (0u64, 0u64);
    unsafe {
        let mut table_ptr: *mut windows::Win32::NetworkManagement::IpHelper::MIB_IF_TABLE2 = std::ptr::null_mut();
        let ret = windows::Win32::NetworkManagement::IpHelper::GetIfTable2(&mut table_ptr);
        if ret.is_ok() && !table_ptr.is_null() {
            let table = &*table_ptr;
            let num_entries = table.NumEntries;
            let rows = table.Table.as_ptr();
            for i in 0..num_entries as usize {
                let row = &*rows.add(i);
                total_in += row.InOctets;
                total_out += row.OutOctets;
            }
            windows::Win32::NetworkManagement::IpHelper::FreeMibTable(table_ptr as *const _);
        }
    }

    let prev_sent = LAST_BYTES_SENT.swap(total_out, Ordering::Relaxed);
    let prev_recv = LAST_BYTES_RECV.swap(total_in, Ordering::Relaxed);
    LAST_CHECK.store(now, Ordering::Relaxed);

    if last_check == 0 {
        return Ok((0, 0));
    }

    let sent_per_sec = if total_out > prev_sent { ((total_out - prev_sent) * 1000) / elapsed } else { 0 };
    let recv_per_sec = if total_in > prev_recv { ((total_in - prev_recv) * 1000) / elapsed } else { 0 };

    Ok((sent_per_sec, recv_per_sec))
}

#[tauri::command]
pub fn get_windows_accent_color() -> Option<String> {
    unsafe {
        use windows::Win32::System::Registry::{RegOpenKeyExW, RegQueryValueExW, RegCloseKey, HKEY_CURRENT_USER, KEY_READ, REG_BINARY, REG_DWORD};
        
        // 1. Try reading AccentPalette from Explorer\Accent for the true system accent color
        let subkey = windows::core::w!("Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Accent");
        let mut hkey = windows::Win32::System::Registry::HKEY::default();
        if RegOpenKeyExW(HKEY_CURRENT_USER, subkey, Some(0), KEY_READ, &mut hkey).is_ok() {
            let mut value_type = REG_BINARY;
            let mut palette = [0u8; 32];
            let mut value_size = 32u32;
            let val_name = windows::core::w!("AccentPalette");
            if RegQueryValueExW(
                hkey,
                val_name,
                None,
                Some(&mut value_type),
                Some(palette.as_mut_ptr()),
                Some(&mut value_size)
            ).is_ok() {
                let _ = RegCloseKey(hkey);
                if value_size >= 15 {
                    // bytes 12, 13, 14 are R, G, B of the active accent color
                    let r = palette[12];
                    let g = palette[13];
                    let b = palette[14];
                    return Some(format!("#{:02x}{:02x}{:02x}", r, g, b));
                }
            } else {
                let _ = RegCloseKey(hkey);
            }
        }

        // 2. Fallback to DWM\AccentColor
        let subkey = windows::core::w!("Software\\Microsoft\\Windows\\DWM");
        let mut hkey = windows::Win32::System::Registry::HKEY::default();
        if RegOpenKeyExW(HKEY_CURRENT_USER, subkey, Some(0), KEY_READ, &mut hkey).is_ok() {
            let mut value_type = REG_DWORD;
            let mut color_val = 0u32;
            let mut value_size = std::mem::size_of::<u32>() as u32;
            let val_name = windows::core::w!("AccentColor");
            if RegQueryValueExW(
                hkey,
                val_name,
                None,
                Some(&mut value_type),
                Some(&mut color_val as *mut u32 as *mut u8),
                Some(&mut value_size)
            ).is_ok() {
                let _ = RegCloseKey(hkey);
                // color_val is AABBGGRR (ABGR format)
                let r = (color_val & 0xff) as u8;
                let g = ((color_val >> 8) & 0xff) as u8;
                let b = ((color_val >> 16) & 0xff) as u8;
                return Some(format!("#{:02x}{:02x}{:02x}", r, g, b));
            }
            let _ = RegCloseKey(hkey);
        }
    }
    None
}

#[tauri::command]
pub fn get_system_accent_color() -> Result<String, String> {
    // Try reading registry first for exact Windows accent color
    if let Some(color) = get_windows_accent_color() {
        return Ok(color);
    }

    // Fallback to DwmGetColorizationColor
    unsafe {
        let mut color = 0u32;
        let mut opaque = windows::core::BOOL(0);
        if windows::Win32::Graphics::Dwm::DwmGetColorizationColor(&mut color, &mut opaque).is_ok() {
            let r = ((color >> 16) & 0xff) as u8;
            let g = ((color >> 8) & 0xff) as u8;
            let b = (color & 0xff) as u8;
            let hex = format!("#{:02x}{:02x}{:02x}", r, g, b);
            Ok(hex)
        } else {
            Err("Failed to query colorization color".into())
        }
    }
}

#[tauri::command]
pub fn export_settings(app: AppHandle) -> Result<String, String> {
    let path = app.path().app_config_dir().map_err(|e| e.to_string())?.join("settings.json");
    if let Ok(content) = std::fs::read_to_string(&path) {
        // Validate it's valid JSON before returning
        let _settings: HashMap<String, serde_json::Value> =
            serde_json::from_str(&content).map_err(|e| format!("Invalid settings file: {}", e))?;
        Ok(content)
    } else {
        Err("No settings file found".into())
    }
}

#[tauri::command]
pub fn read_settings_from_path(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path).map_err(|e| format!("Failed to read file: {}", e))
}

#[tauri::command]
pub fn write_settings_to_path(path: String, content: String) -> Result<(), String> {
    std::fs::write(&path, content).map_err(|e| format!("Failed to write file: {}", e))
}

#[tauri::command]
pub fn import_settings(app: AppHandle, settings: String) -> Result<(), String> {
    let imported: HashMap<String, serde_json::Value> =
        serde_json::from_str(&settings).map_err(|e| format!("Invalid JSON: {}", e))?;

    let path = app.path().app_config_dir().map_err(|e| e.to_string())?.join("settings.json");
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let content = serde_json::to_string_pretty(&imported).map_err(|e| e.to_string())?;
    std::fs::write(&path, content).map_err(|e| e.to_string())?;

    crate::utils::replace_settings_cache(imported.clone());

    // Broadcast changes for every imported key so all windows re-sync
    for (key, value) in &imported {
        let _ = app.emit("settings-changed", serde_json::json!({ "key": key, "value": value }));
    }

    if imported.get("nectar-scale").is_some() {
        re_register_appbars(&app, &imported);
    }

    Ok(())
}

pub fn setup_settings_watcher(app: AppHandle) {
    use tauri::Manager;

    let config_dir = match app.path().app_config_dir() {
        Ok(p) => p,
        Err(_) => return,
    };
    let settings_path = config_dir.join("settings.json");

    std::thread::spawn(move || {
        use windows::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
        use windows::Win32::Storage::FileSystem::{
            CreateFileW, FILE_FLAG_BACKUP_SEMANTICS, FILE_LIST_DIRECTORY,
            FILE_NOTIFY_CHANGE_LAST_WRITE,
            OPEN_EXISTING, ReadDirectoryChangesW,
        };
        use windows::Win32::System::IO::OVERLAPPED;
        use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject, INFINITE};

        unsafe {
            let dir_path: Vec<u16> = config_dir.to_string_lossy()
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();

            let dir_handle = match CreateFileW(
                windows::core::PCWSTR(dir_path.as_ptr()),
                FILE_LIST_DIRECTORY.0,
                windows::Win32::Storage::FileSystem::FILE_SHARE_READ
                    | windows::Win32::Storage::FileSystem::FILE_SHARE_WRITE
                    | windows::Win32::Storage::FileSystem::FILE_SHARE_DELETE,
                None,
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS,
                None,
            ) {
                Ok(h) if h != INVALID_HANDLE_VALUE => h,
                _ => return,
            };

            let h_event = match CreateEventW(None, false, false, None) {
                Ok(e) => e,
                Err(_) => {
                    let _ = CloseHandle(dir_handle);
                    return;
                }
            };

            let mut notify_buffer = [0u8; 4096];
            let mut bytes_returned = 0u32;
            let mut overlapped = OVERLAPPED::default();
            overlapped.hEvent = h_event;

            loop {
                let success = ReadDirectoryChangesW(
                    dir_handle,
                    notify_buffer.as_mut_ptr() as *mut _,
                    notify_buffer.len() as u32,
                    false,
                    FILE_NOTIFY_CHANGE_LAST_WRITE,
                    Some(&mut bytes_returned),
                    Some(&mut overlapped),
                    None,
                );

                if success.is_err() {
                    break;
                }

                WaitForSingleObject(h_event, INFINITE);

                std::thread::sleep(std::time::Duration::from_millis(200));

                if let Ok(new_content) = std::fs::read_to_string(&settings_path) {
                    if let Ok(new_settings) = serde_json::from_str::<HashMap<String, serde_json::Value>>(&new_content) {
                        // Collect diffs while holding the lock, then drop before emitting
                        let (changed, removed) = {
                            let mut cache = crate::state::SETTINGS_CACHE
                                .get_or_init(|| std::sync::Mutex::new(HashMap::new()))
                                .lock()
                                .unwrap();

                            let mut changed = Vec::new();
                            for (key, value) in &new_settings {
                                if cache.get(key) != Some(value) {
                                    changed.push((key.clone(), value.clone()));
                                }
                            }

                            let removed: Vec<String> = cache.keys()
                                .filter(|k| !new_settings.contains_key(*k))
                                .cloned()
                                .collect();

                            *cache = new_settings.clone();
                            (changed, removed)
                        };
                        // Lock dropped — safe to emit without blocking save_setting
                        for (key, value) in &changed {
                            let _ = app.emit(
                                "settings-external-changed",
                                serde_json::json!({ "key": key, "value": value }),
                            );
                        }
                        for key in removed {
                            let _ = app.emit(
                                "settings-external-changed",
                                serde_json::json!({ "key": key, "value": null }),
                            );
                        }
                    }
                }
            }

            let _ = CloseHandle(h_event);
            let _ = CloseHandle(dir_handle);
        }
    });
}

#[cfg(test)]
mod pwa_icon_tests {
    use super::*;

    #[test]
    fn aumid_detection() {
        assert!(is_aumid_path("4DF9E0F8.Netflix_mcm4njqhnhss8!Netflix.App"));
        assert!(is_aumid_path("Microsoft.VisualStudioCode"));
        assert!(is_aumid_path("shell:AppsFolder\\4DF9E0F8.Netflix_mcm4njqhnhss8!Netflix.App"));
        assert!(!is_aumid_path("C:\\Windows\\explorer.exe"));
        assert!(!is_aumid_path("msedge.exe"));
        assert!(!is_aumid_path(""));
    }

    #[test]
    fn command_line_arg_parsing() {
        let args = "--profile-directory=\"Profile 1\" --app-id=abcdef --ip-aumid=Package_Pub!App";
        assert_eq!(extract_arg(args, "--app-id="), Some("abcdef".into()));
        assert_eq!(extract_arg(args, "--profile-directory="), Some("Profile 1".into()));
        assert_eq!(extract_arg(args, "--ip-aumid="), Some("Package_Pub!App".into()));
        assert_eq!(extract_arg(args, "--missing="), None);
        assert_eq!(extract_arg("shell:AppsFolder\\Microsoft.WindowsCalculator_8wekyb3d8bbwe!App", "shell:AppsFolder\\"), Some("Microsoft.WindowsCalculator_8wekyb3d8bbwe!App".into()));
    }

    #[test]
    fn package_family_from_paths() {
        assert_eq!(
            package_family_from_windows_apps_path("C:\\Program Files\\WindowsApps\\5319275A.WhatsAppDesktop_2.2634.101.0_x64__cv1g1gvanyjgm\\WhatsApp.Root.exe").as_deref(),
            Some("5319275A.WhatsAppDesktop_cv1g1gvanyjgm")
        );
        assert_eq!(package_family_from_windows_apps_path("C:\\Windows\\explorer.exe"), None);
        assert_eq!(package_family_from_windows_apps_path("C:\\Program Files\\App\\app.exe"), None);
    }

    #[test]
    fn browser_web_app_ids_from_aumids() {
        assert_eq!(browser_web_app_id_from_aumid("Chrome.edhbnieanoeijlkpgkminebadpibapgm").as_deref(), Some("edhbnieanoeijlkpgkminebadpibapgm"));
        assert_eq!(browser_web_app_id_from_aumid("Chrome._crx_edhbnieanoeijlkpgkminebadpibapgm").as_deref(), Some("edhbnieanoeijlkpgkminebadpibapgm"));
        assert_eq!(browser_web_app_id_from_aumid("4DF9E0F8.Netflix_mcm4njqhnhss8!Netflix.App"), None);
        assert_eq!(browser_web_app_id_from_aumid("MSEdge"), None);
    }

    #[test]
    fn browser_pwa_aumid_detection() {
        assert!(is_browser_pwa_aumid("Chrome.edhbnieanoeijlkpgkminebadpibapgm"));
        assert!(is_browser_pwa_aumid("Brave._crx_edhbnieanoeijlkpgkminebadpibapgm"));
        assert!(is_browser_pwa_aumid("Brave._crx_agimnkijcamfeangaknmldooml"));
        assert!(is_browser_pwa_aumid("4DF9E0F8.Netflix_mcm4njqhnhss8!Netflix.App"));
        assert!(!is_browser_pwa_aumid("MSEdge"));
        assert!(!is_browser_pwa_aumid("Chrome"));
        assert!(!is_browser_pwa_aumid("Brave"));
    }

    #[test]
    fn image_size_scoring_prefers_larger_dimensions() {
        use std::path::Path;
        assert!(image_size_score(Path::new("C:\\x\\Icons\\256.png")) > image_size_score(Path::new("C:\\x\\Icons\\64.png")));
        assert!(image_size_score(Path::new("C:\\x\\512x512.png")) > image_size_score(Path::new("C:\\x\\192x192.png")));
        assert_eq!(image_size_score(Path::new("C:\\x\\icon.png")), 0);
    }

    #[test]
    fn pwa_shortcut_matching() {
        let app = AppInfo {
            name: "YouTube".into(),
            path: "C:\\Users\\x\\Start Menu\\Programs\\YouTube.lnk".into(),
            icon: None,
            is_running: false,
            hwnd: None,
            executable: Some("brave.exe".into()),
            all_hwnds: None,
        };
        assert!(is_pwa_shortcut_for(&app, "youtube", "brave.exe"));
        assert!(is_pwa_shortcut_for(&app, "YouTube", "BRAVE.EXE"));
        assert!(!is_pwa_shortcut_for(&app, "YouTube", "msedge.exe"));
        assert!(!is_pwa_shortcut_for(&app, "YouTube Music", "brave.exe"));
    }

    #[test]
    fn pwa_launch_target_resolution() {
        // Non-browser paths and missing titles don't resolve.
        assert_eq!(pwa_launch_target("C:\\Windows\\notepad.exe", Some("Notepad")), None);
        assert_eq!(pwa_launch_target("C:\\Program Files\\BraveSoftware\\Brave-Browser\\Application\\brave.exe", None), None);
        assert_eq!(pwa_launch_target("C:\\Program Files\\BraveSoftware\\Brave-Browser\\Application\\brave.exe", Some("   ")), None);

        // With the cache filled, a running PWA resolves to its shortcut, a same-titled app in another browser doesn't.
        let _ = INSTALLED_APPS_CACHE.set(std::sync::Mutex::new(vec![
            AppInfo {
                name: "YouTube".into(),
                path: "C:\\Start Menu\\YouTube.lnk".into(),
                icon: None,
                is_running: false,
                hwnd: None,
                executable: Some("brave.exe".into()),
                all_hwnds: None,
            },
            AppInfo {
                name: "Netflix".into(),
                path: "C:\\Start Menu\\Netflix.lnk".into(),
                icon: None,
                is_running: false,
                hwnd: None,
                executable: Some("msedge.exe".into()),
                all_hwnds: None,
            },
        ]));
        assert_eq!(
            pwa_launch_target("C:\\Program Files\\BraveSoftware\\Brave-Browser\\Application\\brave.exe", Some("YouTube")),
            Some("C:\\Start Menu\\YouTube.lnk".into())
        );
        assert_eq!(
            pwa_launch_target("C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe", Some("YouTube")),
            None
        );
        assert_eq!(
            pwa_launch_target("C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe", Some("Netflix")),
            Some("C:\\Start Menu\\Netflix.lnk".into())
        );
    }
}
