//! Reading the notification area, so the dock can draw its own tray.
//!
//! Windows 11's taskbar is XAML and exposes no icon list: there is no `ToolbarWindow32` to read
//! `TBBUTTON`s from, and UI Automation on `Shell_TrayWnd` returns nothing. What still works is
//! [`Shell_NotifyIconGetRect`]: for a given notification-area window and icon id it returns the
//! icon's rectangle on screen.
//!
//! Every tray icon is owned by a window its process created for `Shell_NotifyIcon`, and the
//! window class usually says so — `WxTrayIconMessageWindow`, `Electron_NotifyIconHostWindow`,
//! `Chrome_StatusTrayWindow`, `tray_icon_app`, and so on. Enumerating windows for those classes
//! and probing icon ids 0..64 finds them all, and the rectangles give us both the pixel to click
//! and the identity of the process behind each icon.
//!
//! Clicking is done by moving the cursor onto the icon's real rectangle and clicking there: a
//! tray icon only responds to messages its own window receives, and the shell routes those from
//! a genuine click. That is also how the shell's own notification area works, so context menus
//! and balloons behave normally.

use std::collections::HashSet;
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use windows::Win32::Foundation::{HWND, POINT, RECT};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    keybd_event, mouse_event, KEYEVENTF_KEYUP, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP,
    MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, VK_ESCAPE,
};
use windows::Win32::UI::Shell::{Shell_NotifyIconGetRect, NOTIFYICONIDENTIFIER};
use windows::Win32::Foundation::LPARAM;
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, GetWindowThreadProcessId, IsWindow, SetCursorPos,
};

/// Icon ids are small and assigned by the owning process; 64 covers anything reasonable.
const MAX_ICON_ID: u32 = 64;

#[derive(Clone, Serialize)]
pub struct TrayIcon {
    /// Identifies this icon across refreshes: the notification-area window plus the icon id.
    pub key: String,
    /// Process that registered the icon, used for the display name and the pixel to click.
    pub pid: u32,
    /// Executable name without the extension, e.g. "Weixin".
    pub app: String,
    /// Friendly name from the executable's version info, when it has one.
    pub name: String,
    /// Full path to the owning executable, so the dock can resolve its icon with the same code
    /// the rest of the dock uses. The tray icon bitmap itself lives inside the shell's UI and
    /// cannot be read.
    pub path: String,
    pub left: i32,
    pub top: i32,
    pub width: i32,
    pub height: i32,
    /// Whether the icon is inside the always-visible part of the notification area rather than
    /// the overflow flyout. Either way the rectangle is usable; this only informs ordering.
    pub promoted: bool,
    /// The app is asking for attention: one of its windows flashed, or its unread count went up.
    /// The dock pulses the icon until the user opens the app.
    pub attention: bool,
}

/// Notification-area windows found last scan, used to find the icon under the cursor without
/// walking every window again.
static LAST_SCAN: Mutex<Vec<(isize, u32, TrayIcon)>> = Mutex::new(Vec::new());

/// A window class that suggests the window exists to host a notification icon.
fn looks_like_tray_host(class_name: &str) -> bool {
    let lower = class_name.to_ascii_lowercase();
    lower.contains("tray")
        || lower.contains("notifyicon")
        || lower.contains("statusicon")
        || lower.contains("systemtray")
}

/// Every notification-area icon currently registered, ordered left to right.
pub fn enumerate() -> Vec<TrayIcon> {
    struct Ctx {
        windows: Vec<HWND>,
    }

    unsafe extern "system" fn collect(hwnd: HWND, lparam: LPARAM) -> windows::core::BOOL {
        let ctx = &mut *(lparam.0 as *mut Ctx);
        if !IsWindow(Some(hwnd)).as_bool() {
            return true.into();
        }
        let mut buf = [0u16; 256];
        let len = GetClassNameW(hwnd, &mut buf);
        if len <= 0 {
            return true.into();
        }
        let class = String::from_utf16_lossy(&buf[..len as usize]);
        if looks_like_tray_host(&class) {
            ctx.windows.push(hwnd);
        }
        true.into()
    }

    let mut ctx = Ctx { windows: Vec::new() };
    unsafe {
        let _ = EnumWindows(Some(collect), LPARAM(&mut ctx as *mut Ctx as isize));
    }

    let mut icons: Vec<TrayIcon> = Vec::new();
    let mut seen: HashSet<(isize, u32)> = HashSet::new();

    for hwnd in ctx.windows {
        let mut pid = 0u32;
        unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
        if pid == 0 || pid == std::process::id() {
            continue;
        }

        for id in 0..MAX_ICON_ID {
            let Some(rect) = icon_rect(hwnd, id) else { continue };
            let key = (hwnd.0 as isize, id);
            if !seen.insert(key) {
                continue;
            }

            let width = rect.right - rect.left;
            let height = rect.bottom - rect.top;
            // A hidden icon reports an empty or off-screen rectangle; only visible ones matter.
            if width <= 0 || height <= 0 {
                continue;
            }

            let (app, name, path) = process_identity(pid);
            icons.push(TrayIcon {
                key: format!("{}:{}", key.0, id),
                pid,
                app,
                name,
                path,
                left: rect.left,
                top: rect.top,
                width,
                height,
                promoted: true,
                attention: crate::attention::process_wants_attention(pid),
            });
        }
    }

    icons.sort_by_key(|icon| (icon.promoted, icon.left));
    if let Ok(mut cache) = LAST_SCAN.lock() {
        *cache = icons
            .iter()
            .map(|icon| (icon.pid as isize, 0u32, icon.clone()))
            .collect();
    }
    icons
}

/// The screen rectangle of icon `id` on `hwnd`, or `None` when that id has no icon.
fn icon_rect(hwnd: HWND, id: u32) -> Option<RECT> {
    let identifier = NOTIFYICONIDENTIFIER {
        cbSize: std::mem::size_of::<NOTIFYICONIDENTIFIER>() as u32,
        hWnd: hwnd,
        uID: id,
        ..Default::default()
    };
    unsafe { Shell_NotifyIconGetRect(&identifier) }.ok()
}

/// Executable stem, friendly name, and full path for a process.
fn process_identity(pid: u32) -> (String, String, String) {
    let Some(path) = process_path(pid) else {
        return (format!("PID {pid}"), String::new(), String::new());
    };
    let app = std::path::Path::new(&path)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| path.clone());
    let name = crate::utils::exe_description(&path).unwrap_or_else(|| app.clone());
    (app, name, path)
}

fn process_path(pid: u32) -> Option<String> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            windows::core::PWSTR(buf.as_mut_ptr()),
            &mut len,
        );
        let _ = windows::Win32::Foundation::CloseHandle(handle);
        ok.ok()?;
        Some(String::from_utf16_lossy(&buf[..len as usize]))
    }
}

/// Sends a real click to the icon's own pixels. `right` opens the icon's context menu, which is
/// where "Safely Remove Hardware" and similar entries live.
///
/// The cursor is put back afterwards so a click from the dock does not leave the pointer parked
/// over the notification area.
pub fn click(icon: &TrayIcon, right: bool) -> Result<(), String> {
    let x = icon.left + icon.width / 2;
    let y = icon.top + icon.height / 2;

    // The icon can move between the scan and the click (another icon appearing, the flyout
    // closing). Clicking a stale coordinate would hit whatever is there now, so confirm the
    // rectangle is still on a monitor.
    let on_screen = unsafe {
        windows::Win32::Graphics::Gdi::MonitorFromPoint(
            POINT { x, y },
            windows::Win32::Graphics::Gdi::MONITOR_DEFAULTTONULL,
        )
    };
    if on_screen.is_invalid() {
        return Err("that tray icon has moved".into());
    }

    unsafe {
        let mut previous = POINT::default();
        if windows::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut previous).is_err() {
            return Err("couldn't read the cursor position".into());
        }

        let _ = SetCursorPos(x, y);
        std::thread::sleep(Duration::from_millis(30));
        let (down, up) = if right {
            (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP)
        } else {
            (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP)
        };
        mouse_event(down, 0, 0, 0, 0);
        std::thread::sleep(Duration::from_millis(30));
        mouse_event(up, 0, 0, 0, 0);
        std::thread::sleep(Duration::from_millis(60));
        let _ = SetCursorPos(previous.x, previous.y);
    }
    Ok(())
}

/// Closes whatever popup the tray icon opened, the way pressing Escape on the desktop does.
pub fn dismiss_popups() {
    unsafe {
        keybd_event(VK_ESCAPE.0 as u8, 0, Default::default(), 0);
        keybd_event(VK_ESCAPE.0 as u8, 0, KEYEVENTF_KEYUP, 0);
    }
}

/// Looks up an icon from the last scan, so a click handler does not have to scan again.
pub fn find(key: &str) -> Option<TrayIcon> {
    let cache = LAST_SCAN.lock().ok()?;
    cache.iter().find(|(_, _, icon)| icon.key == key).map(|(_, _, icon)| icon.clone())
}

#[cfg(test)]
mod tests {
    use super::looks_like_tray_host;

    #[test]
    fn recognises_real_tray_host_class_names() {
        // Observed on a Windows 11 machine running WeChat, Slack and others.
        assert!(looks_like_tray_host("Qt51514WxTrayIconMessageWindowClass"));
        assert!(looks_like_tray_host("Electron_NotifyIconHostWindow"));
        assert!(looks_like_tray_host("Chrome_StatusTrayWindow"));
        assert!(looks_like_tray_host("tray_icon_app"));
        assert!(looks_like_tray_host("SystemTray_Main"));
        assert!(looks_like_tray_host("PToyTrayIconWindow"));
        assert!(looks_like_tray_host("MSRDCSysTrayClass_WSL"));
    }

    #[test]
    fn ignores_ordinary_application_windows() {
        assert!(!looks_like_tray_host("Chrome_WidgetWin_1"));
        assert!(!looks_like_tray_host("CabinetWClass"));
        assert!(!looks_like_tray_host("Notepad"));
        assert!(!looks_like_tray_host("Tauri Window"));
        assert!(!looks_like_tray_host(""));
    }
}

/// Whether `path` is running with a window that is hidden rather than absent.
pub fn has_hidden_window(path: &str) -> bool {
    find_hidden_window(path).is_some()
}

/// Finds a hidden window belonging to a given executable and shows it.
///
/// Used when a pinned dock app is clicked while it is running with no window: WeChat and similar
/// apps hide their main window when closed, and launching the executable again would start a
/// second copy — a second login prompt — instead of bringing the running app back.
pub fn restore_hidden_window(path: &str) -> bool {
    let Some(hwnd) = find_hidden_window(path) else { return false };
    unsafe {
        let _ = windows::Win32::UI::WindowsAndMessaging::ShowWindow(
            hwnd,
            windows::Win32::UI::WindowsAndMessaging::SW_RESTORE,
        );
        let _ = windows::Win32::UI::WindowsAndMessaging::ShowWindow(
            hwnd,
            windows::Win32::UI::WindowsAndMessaging::SW_SHOW,
        );
        let _ = windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow(hwnd);
    }
    true
}

/// The hidden window of `path`, if it has one.
///
/// The window has to look like a real app window and must not be a notification-area host: those
/// also carry a caption and a title, and restoring one would show nothing.
fn find_hidden_window(path: &str) -> Option<HWND> {
    struct Ctx {
        wanted: String,
        found: Option<HWND>,
    }

    unsafe extern "system" fn visit(hwnd: HWND, lparam: LPARAM) -> windows::core::BOOL {
        let ctx = &mut *(lparam.0 as *mut Ctx);
        if ctx.found.is_some() {
            return false.into();
        }
        if windows::Win32::UI::WindowsAndMessaging::IsWindowVisible(hwnd).as_bool() {
            return true.into();
        }
        // Owned windows are dialogs and palettes, not the app's main window.
        let owner = windows::Win32::UI::WindowsAndMessaging::GetWindow(hwnd, windows::Win32::UI::WindowsAndMessaging::GW_OWNER)
            .unwrap_or_default();
        if !owner.0.is_null() {
            return true.into();
        }
        let ex_style = windows::Win32::UI::WindowsAndMessaging::GetWindowLongW(
            hwnd,
            windows::Win32::UI::WindowsAndMessaging::GWL_EXSTYLE,
        ) as u32;
        if ex_style & windows::Win32::UI::WindowsAndMessaging::WS_EX_TOOLWINDOW.0 != 0 {
            return true.into();
        }
        let style = windows::Win32::UI::WindowsAndMessaging::GetWindowLongW(
            hwnd,
            windows::Win32::UI::WindowsAndMessaging::GWL_STYLE,
        ) as u32;
        if style & windows::Win32::UI::WindowsAndMessaging::WS_CAPTION.0 == 0 {
            return true.into();
        }

        let mut class_buf = [0u16; 256];
        let class_len = windows::Win32::UI::WindowsAndMessaging::GetClassNameW(hwnd, &mut class_buf);
        if class_len > 0 {
            let class = String::from_utf16_lossy(&class_buf[..class_len as usize]);
            // A notification-area host has no size of its own and is not the app's window.
            if looks_like_tray_host(&class) {
                return true.into();
            }
        }

        let mut pid = 0u32;
        windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 || pid == std::process::id() {
            return true.into();
        }
        let Some(process_exe) = process_path(pid) else { return true.into() };
        if !process_exe.eq_ignore_ascii_case(&ctx.wanted) {
            return true.into();
        }

        ctx.found = Some(hwnd);
        false.into()
    }

    let mut ctx = Ctx { wanted: path.to_string(), found: None };
    unsafe {
        let _ = EnumWindows(Some(visit), LPARAM(&mut ctx as *mut Ctx as isize));
    }

    ctx.found
}
