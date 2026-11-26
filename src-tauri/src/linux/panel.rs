//! Hides the desktop's own panel while the Nectar dock replaces it, the Linux
//! counterpart of hiding the Windows taskbar. Only Cinnamon is handled: its panels
//! are switched to auto-hide with a very long show delay, so hovering the screen
//! edge reveals the Nectar dock instead of the panel. The Super key menu still
//! works. The previous values are restored on exit and also kept in a file, so a
//! crash or kill doesn't leave the panel hidden.

use crate::utils::run;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

static HIDDEN: AtomicBool = AtomicBool::new(false);
static BACKUP_PATH: Mutex<Option<std::path::PathBuf>> = Mutex::new(None);

/// Restore order matters: Cinnamon shows a panel whose auto-hide is turned off only
/// after the show delay, so the delay has to be back to normal first.
const KEYS: [&str; 2] = ["panels-show-delay", "panels-autohide"];
/// One day: the panel never pops up from a hover.
const NEVER_MS: &str = "86400000";

fn is_cinnamon() -> bool {
    std::env::var("XDG_CURRENT_DESKTOP").is_ok_and(|d| d.to_lowercase().contains("cinnamon"))
}

pub fn init(config_dir: std::path::PathBuf) {
    if let Ok(mut p) = BACKUP_PATH.lock() {
        *p = Some(config_dir.join("panel_backup.json"));
    }
}

fn backup_path() -> Option<std::path::PathBuf> {
    BACKUP_PATH.lock().ok()?.clone()
}

fn gset(key: &str, value: &str) {
    let _ = std::process::Command::new("gsettings").args(["set", "org.cinnamon", key, value]).status();
}

/// Puts back values from a backup left behind. Returns true when there was one,
/// which means the last session ended without restoring the panel.
pub fn restore_after_crash() -> bool {
    let Some(path) = backup_path() else { return false };
    let Ok(content) = std::fs::read_to_string(&path) else { return false };
    if is_cinnamon() {
        if let Ok(saved) = serde_json::from_str::<std::collections::HashMap<String, String>>(&content) {
            for key in KEYS {
                if let Some(v) = saved.get(key).filter(|v| !v.trim().is_empty()) {
                    gset(key, v.trim());
                }
            }
        }
    }
    let _ = std::fs::remove_file(path);
    true
}

/// Panel ids from a value like "['1:false', '2:true']".
fn panel_ids(value: &str) -> Vec<String> {
    value
        .trim_matches(|c| c == '[' || c == ']')
        .split(',')
        .filter_map(|e| e.trim().trim_matches('\'').split(':').next().map(String::from))
        .filter(|id| !id.is_empty() && !id.starts_with('@'))
        .collect()
}

fn per_panel(ids: &[String], value: &str) -> String {
    format!("[{}]", ids.iter().map(|id| format!("'{id}:{value}'")).collect::<Vec<_>>().join(", "))
}

/// `visible == false` hides every Cinnamon panel from edge hovers.
pub fn set_visible(visible: bool) {
    if !is_cinnamon() {
        return;
    }
    if !visible {
        if crate::utils::setting_str("nectar-linux-hide-panel").as_deref() == Some("false") {
            return;
        }
        if HIDDEN.swap(true, Ordering::SeqCst) {
            return;
        }
        let mut saved = std::collections::HashMap::new();
        for key in KEYS {
            if let Some(v) = run("gsettings", &["get", "org.cinnamon", key]) {
                saved.insert(key.to_string(), v);
            }
        }
        let Some(ids) = saved.get("panels-autohide").map(|v| panel_ids(v)).filter(|ids| !ids.is_empty()) else { return };
        if let Some(path) = backup_path() {
            // Keep the oldest backup: it holds the user's own values, not ours.
            if !path.exists() {
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let _ = std::fs::write(&path, serde_json::to_string(&saved).unwrap_or_default());
            }
        }
        gset("panels-show-delay", &per_panel(&ids, NEVER_MS));
        gset("panels-autohide", &per_panel(&ids, "true"));
        crate::diagnostics::log("panel: Cinnamon panels hidden while the dock is on");
    } else {
        if !HIDDEN.swap(false, Ordering::SeqCst) {
            return;
        }
        restore_after_crash();
        crate::diagnostics::log("panel: Cinnamon panels restored");
    }
}
