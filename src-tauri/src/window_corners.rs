//! Rounding the frameless settings and calendar windows.
//!
//! These windows are `decorations: false`, `transparent: true`, and get an acrylic backdrop
//! from `window_vibrancy`. That backdrop is painted by DWM across the whole window rectangle and
//! CSS cannot clip it — a `border-radius` on the panel rounds the panel, not the acrylic.
//!
//! So DWM has to round the window, and the CSS radius has to line up with it. DWM's radius for a
//! normal top-level window is about 8 px and is not configurable; `DWMWA_WINDOW_CORNER_PREFERENCE`
//! only offers off, 8 px, and a smaller 4 px. That gives one rule:
//!
//!   CSS radius > DWM radius   a crescent of acrylic shows at each corner
//!   CSS radius <= DWM radius  the panel covers everything DWM keeps
//!
//! Hence the settings and calendar panels use 8 px, matching the system. Turning DWM rounding
//! off entirely is not an option: the acrylic rectangle then shows square at all four corners.
//!
//! No code is needed on Windows 10, which does not round frameless windows at all.

use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
};

/// Corner radius of the settings and calendar panels, in logical pixels. Must not exceed the
/// radius Windows gives a top-level window, or the acrylic behind it shows at the corners.
pub const PANEL_CORNER_RADIUS: f64 = 8.0;

/// Asks DWM to keep its standard rounding. It is the default for a frameless window on Windows
/// 11, so this mainly documents the assumption and pins it against a future default change.
pub fn round_like_system(hwnd: HWND) {
    unsafe {
        let preference = DWMWCP_ROUND;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &preference as *const _ as *const core::ffi::c_void,
            std::mem::size_of_val(&preference) as u32,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::PANEL_CORNER_RADIUS;

    #[test]
    fn corner_preference_constants_are_what_the_comments_claim() {
        // A typo in either constant makes the call fail silently, leaving the window on
        // whatever the default happens to be.
        assert_eq!(windows::Win32::Graphics::Dwm::DWMWA_WINDOW_CORNER_PREFERENCE.0, 33);
        assert_eq!(windows::Win32::Graphics::Dwm::DWMWCP_ROUND.0, 2);
    }

    #[test]
    fn panel_radius_does_not_exceed_the_system_radius() {
        // Keeps Settings.css and Calendar.css honest: they use this same 8 px. Raising the CSS
        // radius alone brings the acrylic crescent back.
        assert_eq!(PANEL_CORNER_RADIUS, 8.0);
    }
}
