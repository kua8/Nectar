use std::sync::{atomic::{AtomicBool, AtomicU32, AtomicI64}, Mutex, OnceLock};
use std::sync::mpsc::Sender;
use std::collections::HashMap;
use crate::types::{SystemCommand, IntRect, AppInfo};
use tauri::{AppHandle, PhysicalPosition, PhysicalSize};

pub static COMMAND_SENDER: OnceLock<Sender<SystemCommand>> = OnceLock::new();
pub static NATIVE_TASKBAR_HIDDEN: AtomicBool = AtomicBool::new(false);

macro_rules! label_map {
    ($fn_name:ident, $val_ty:ty) => {
        pub fn $fn_name() -> &'static Mutex<HashMap<String, $val_ty>> {
            static CELL: OnceLock<Mutex<HashMap<String, $val_ty>>> = OnceLock::new();
            CELL.get_or_init(|| Mutex::new(HashMap::new()))
        }
    };
}

label_map!(dock_rects, IntRect);
label_map!(notch_rects, IntRect);
label_map!(dock_hovered, bool);
label_map!(notch_hovered, bool);
label_map!(dock_overlap, i32);
label_map!(notch_overlap, i32);
label_map!(dock_window_rects, (PhysicalPosition<i32>, PhysicalSize<u32>));
label_map!(main_window_rects, (PhysicalPosition<i32>, PhysicalSize<u32>));
label_map!(dock_appbar_registered, bool);
label_map!(main_appbar_registered, bool);

label_map!(dock_extra_last_ignore, i32);
label_map!(notch_extra_last_ignore, i32);
label_map!(dock_extra_last_edge_hover, i32);
label_map!(notch_extra_last_edge_hover, i32);
label_map!(dock_extra_expiry_ms, i64);
label_map!(notch_extra_expiry_ms, i64);

pub fn get_flag(map: &'static Mutex<HashMap<String, bool>>, label: &str) -> bool {
    map.lock().ok().and_then(|m| m.get(label).copied()).unwrap_or(false)
}

pub fn set_flag(map: &'static Mutex<HashMap<String, bool>>, label: &str, value: bool) {
    if let Ok(mut m) = map.lock() {
        m.insert(label.to_string(), value);
    }
}

pub fn get_overlap(map: &'static Mutex<HashMap<String, i32>>, label: &str) -> i32 {
    map.lock().ok().and_then(|m| m.get(label).copied()).unwrap_or(-1)
}

pub fn set_overlap(map: &'static Mutex<HashMap<String, i32>>, label: &str, value: i32) {
    if let Ok(mut m) = map.lock() {
        m.insert(label.to_string(), value);
    }
}

pub fn get_i32(map: &'static Mutex<HashMap<String, i32>>, label: &str, default: i32) -> i32 {
    map.lock().ok().and_then(|m| m.get(label).copied()).unwrap_or(default)
}

pub fn set_i32(map: &'static Mutex<HashMap<String, i32>>, label: &str, value: i32) {
    if let Ok(mut m) = map.lock() {
        m.insert(label.to_string(), value);
    }
}

pub fn get_i64(map: &'static Mutex<HashMap<String, i64>>, label: &str, default: i64) -> i64 {
    map.lock().ok().and_then(|m| m.get(label).copied()).unwrap_or(default)
}

pub fn set_i64(map: &'static Mutex<HashMap<String, i64>>, label: &str, value: i64) {
    if let Ok(mut m) = map.lock() {
        m.insert(label.to_string(), value);
    }
}

pub fn clear_window_state(label: &str) {
    if let Ok(mut m) = dock_rects().lock() { m.remove(label); }
    if let Ok(mut m) = notch_rects().lock() { m.remove(label); }
    if let Ok(mut m) = dock_hovered().lock() { m.remove(label); }
    if let Ok(mut m) = notch_hovered().lock() { m.remove(label); }
    if let Ok(mut m) = dock_overlap().lock() { m.remove(label); }
    if let Ok(mut m) = notch_overlap().lock() { m.remove(label); }
    if let Ok(mut m) = dock_window_rects().lock() { m.remove(label); }
    if let Ok(mut m) = main_window_rects().lock() { m.remove(label); }
    if let Ok(mut m) = dock_appbar_registered().lock() { m.remove(label); }
    if let Ok(mut m) = main_appbar_registered().lock() { m.remove(label); }
    if let Ok(mut m) = dock_extra_last_ignore().lock() { m.remove(label); }
    if let Ok(mut m) = notch_extra_last_ignore().lock() { m.remove(label); }
    if let Ok(mut m) = dock_extra_last_edge_hover().lock() { m.remove(label); }
    if let Ok(mut m) = notch_extra_last_edge_hover().lock() { m.remove(label); }
    if let Ok(mut m) = dock_extra_expiry_ms().lock() { m.remove(label); }
    if let Ok(mut m) = notch_extra_expiry_ms().lock() { m.remove(label); }
}

pub fn dock_label_for_monitor(monitor_id: &str) -> String {
    format!("dock-{monitor_id}")
}

pub fn notch_label_for_monitor(monitor_id: &str) -> String {
    format!("main-{monitor_id}")
}

pub fn is_dock_label(label: &str) -> bool {
    label == "dock" || label.starts_with("dock-")
}

pub fn is_notch_label(label: &str) -> bool {
    label == "main" || label.starts_with("main-")
}

pub fn monitor_suffix(label: &str) -> Option<&str> {
    label.split_once('-').map(|(_, id)| id)
}

pub static MONITOR_SYNC_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

pub static MENU_IS_OPEN: AtomicBool = AtomicBool::new(false);
pub static MENU_RECT: Mutex<Option<IntRect>> = Mutex::new(None);
// Set while the user is drag-reordering a pinned dock icon. The mouse hook's
// click-through hit-test region is tight (icon-sized + small hysteresis), and a
// real drag gesture can easily carry the cursor outside it — if that happens
// mid-drag, the dock window goes click-through and stops receiving mouse
// events entirely, silently killing the drag with no pointerup ever delivered.
// This flag forces the dock fully interactive for the gesture's duration.
pub static DOCK_IS_DRAGGING: AtomicBool = AtomicBool::new(false);
pub static ICON_CACHE: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();

pub static INSTALLED_APPS_CACHE: OnceLock<Mutex<Vec<AppInfo>>> = OnceLock::new();
pub static IS_SCANNING: AtomicBool = AtomicBool::new(false);

pub static BRIGHTNESS_SENDER: OnceLock<Sender<u32>> = OnceLock::new();
pub static CURRENT_BRIGHTNESS: AtomicU32 = AtomicU32::new(50);
pub static CURRENT_VOLUME: AtomicU32 = AtomicU32::new(50);
pub static LAST_BRIGHTNESS_CHANGE: AtomicI64 = AtomicI64::new(0);
pub static ANY_MEDIA_PLAYING: AtomicBool = AtomicBool::new(false);
pub static OVERLAY_IN_SPLASH: AtomicBool = AtomicBool::new(false);
pub static CURRENT_FOREGROUND_FULLSCREEN: AtomicBool = AtomicBool::new(false);

pub static SINGLE_INSTANCE_MUTEX_HANDLE: OnceLock<isize> = OnceLock::new();
pub static SINGLE_INSTANCE_EVENT_HANDLE: OnceLock<isize> = OnceLock::new();

pub fn close_single_instance_handles() {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::Foundation::HANDLE;
    if let Some(&h) = SINGLE_INSTANCE_MUTEX_HANDLE.get() {
        if h != 0 {
            unsafe { let _ = CloseHandle(HANDLE(h as *mut _)); }
        }
    }
    if let Some(&h) = SINGLE_INSTANCE_EVENT_HANDLE.get() {
        if h != 0 {
            unsafe { let _ = CloseHandle(HANDLE(h as *mut _)); }
        }
    }
}

pub static DISPLAY_MONITOR_HANDLE: OnceLock<AppHandle> = OnceLock::new();
pub static LAST_DISPLAY_CHANGE_MS: AtomicI64 = AtomicI64::new(0);

pub static THUMBNAIL_CACHE: OnceLock<Mutex<HashMap<isize, (String, i64)>>> = OnceLock::new();
pub static FOCUS_TIMESTAMPS: OnceLock<Mutex<HashMap<isize, i64>>> = OnceLock::new();
pub static SETTINGS_CACHE: OnceLock<Mutex<HashMap<String, serde_json::Value>>> = OnceLock::new();
