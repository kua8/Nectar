//! Which window may call which command.
//!
//! Tauri capabilities only cover plugin commands. Every `#[tauri::command]` in this crate is
//! callable from every window, so a notch, dock, calendar or settings page that ever got a way
//! to evaluate unexpected script would inherit the whole native surface — process killing,
//! launching arbitrary paths, uninstalling. The commands themselves check the label they were
//! invoked from, so each window only reaches what its own UI uses.

use tauri::Window;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Notch,
    Dock,
    Settings,
    Calendar,
    Overlay,
}

pub fn role_of(label: &str) -> Option<Role> {
    if crate::state::is_notch_label(label) {
        Some(Role::Notch)
    } else if crate::state::is_dock_label(label) {
        Some(Role::Dock)
    } else {
        match label {
            "settings" => Some(Role::Settings),
            "calendar" => Some(Role::Calendar),
            "overlay" => Some(Role::Overlay),
            _ => None,
        }
    }
}

/// Refuse the call unless the invoking window has one of the allowed roles. The error text is
/// deliberately vague: it tells a legitimately mismatched caller nothing useful, and gives a
/// page that is probing the IPC surface nothing to work with either.
pub fn require(window: &Window, allowed: &[Role]) -> Result<(), String> {
    let label = window.label().to_string();
    match role_of(&label) {
        Some(role) if allowed.contains(&role) => Ok(()),
        _ => {
            crate::diagnostics::log(&format!("ipc: refused a command from window \"{label}\""));
            Err("This action isn't available in this window.".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{role_of, Role};

    #[test]
    fn recognises_every_window_label_nectar_creates() {
        assert!(role_of("main") == Some(Role::Notch));
        assert!(role_of("main-DISPLAY1") == Some(Role::Notch));
        assert!(role_of("dock") == Some(Role::Dock));
        assert!(role_of("dock-DISPLAY2") == Some(Role::Dock));
        assert!(role_of("settings") == Some(Role::Settings));
        assert!(role_of("calendar") == Some(Role::Calendar));
        assert!(role_of("overlay") == Some(Role::Overlay));
        assert!(role_of("installer").is_none());
        assert!(role_of("").is_none());
    }

    #[test]
    fn a_main_prefix_does_not_swallow_unrelated_labels() {
        // is_notch_label matches `main` and `main-…`; a label that merely contains it must not.
        assert!(role_of("domain-main").is_none());
        assert!(role_of("docks").is_none());
    }
}
