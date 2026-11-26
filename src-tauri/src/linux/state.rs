//! Global state for the Linux backend.
use crate::types::IntRect;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicI64};
use std::sync::{Mutex, OnceLock};

pub static SHUTTING_DOWN: AtomicBool = AtomicBool::new(false);
/// This launch follows a restart or a crash, so calendars should sync right away.
pub static RELAUNCHED: AtomicBool = AtomicBool::new(false);
pub static SPLASH_FINISHED: AtomicBool = AtomicBool::new(false);
pub static OVERLAY_IN_SPLASH: AtomicBool = AtomicBool::new(false);

pub static MENU_IS_OPEN: AtomicBool = AtomicBool::new(false);
pub static MENU_RECT: Mutex<Option<IntRect>> = Mutex::new(None);
pub static DOCK_IS_DRAGGING: AtomicBool = AtomicBool::new(false);
pub static DOCK_HOVERED: AtomicBool = AtomicBool::new(false);
pub static NOTCH_HOVERED: AtomicBool = AtomicBool::new(false);
pub static NOTCH_HIDDEN: AtomicBool = AtomicBool::new(false);

/// Interactive areas reported by the frontend, in CSS px relative to the window.
pub static DOCK_RECT: Mutex<Option<IntRect>> = Mutex::new(None);
pub static DOCK_OVERLAP_RECT: Mutex<Option<IntRect>> = Mutex::new(None);
pub static NOTCH_RECT: Mutex<Option<IntRect>> = Mutex::new(None);

/// Mixer panel bounds (x, y, w, h) in overlay CSS px, None when collapsed.
pub static VOLUME_MIXER_RECT: Mutex<Option<(f64, f64, f64, f64)>> = Mutex::new(None);

pub static CURRENT_FOREGROUND_FULLSCREEN: AtomicBool = AtomicBool::new(false);
pub static CURRENT_FOREGROUND_MAXIMIZED: AtomicBool = AtomicBool::new(false);
pub static LAST_FOCUS_CHANGE_MS: AtomicI64 = AtomicI64::new(0);

pub static SETTINGS_CACHE: OnceLock<Mutex<HashMap<String, serde_json::Value>>> = OnceLock::new();
/// Last time each window was focused, ms since epoch.
pub static FOCUS_TIMESTAMPS: OnceLock<Mutex<HashMap<isize, i64>>> = OnceLock::new();
pub static ICON_CACHE: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();

pub fn focus_timestamps() -> &'static Mutex<HashMap<isize, i64>> {
    FOCUS_TIMESTAMPS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn icon_cache() -> &'static Mutex<HashMap<String, String>> {
    ICON_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}
