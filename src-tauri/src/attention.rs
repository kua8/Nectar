// Apps that want attention, for the dots on dock icons. Chat apps and the like ask Windows to
// flash their taskbar button (FlashWindowEx) when something comes in. Nectar hides the taskbar,
// so it listens to the same shell messages and remembers those windows until they're activated
// or closed, like the taskbar does.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};
use tauri::{AppHandle, Emitter};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    GetAncestor, GetForegroundWindow, GetWindowTextW, IsWindow, GA_ROOTOWNER,
};

const HSHELL_WINDOWDESTROYED: usize = 2;
const HSHELL_WINDOWACTIVATED: usize = 4;
const HSHELL_REDRAW: usize = 6;
const HSHELL_RUDEAPPACTIVATED: usize = 0x8004;
const HSHELL_FLASH: usize = 0x8006;

static APP: OnceLock<AppHandle> = OnceLock::new();
static SHELLHOOK: AtomicU32 = AtomicU32::new(0);
static FLASHING: Mutex<Vec<isize>> = Mutex::new(Vec::new());
// Unread count last seen in each window's title, to tell the dock when one changes.
static COUNTS: OnceLock<Mutex<HashMap<isize, Option<u32>>>> = OnceLock::new();

pub fn start(app: AppHandle) {
    let _ = APP.set(app);
    std::thread::spawn(|| unsafe {
        use windows::Win32::System::LibraryLoader::GetModuleHandleW;
        use windows::Win32::UI::WindowsAndMessaging::{
            CreateWindowExW, DispatchMessageW, GetMessageW, RegisterClassW, RegisterShellHookWindow,
            RegisterWindowMessageW, MSG, WINDOW_STYLE, WNDCLASSW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
        };
        let class = windows::core::w!("NectarAttentionHook");
        let instance = GetModuleHandleW(None).unwrap_or_default().into();
        let wc = WNDCLASSW { lpfnWndProc: Some(shell_proc), hInstance: instance, lpszClassName: class, ..Default::default() };
        if RegisterClassW(&wc) == 0 {
            return;
        }
        let Ok(hwnd) = CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            class,
            windows::core::PCWSTR::null(),
            WINDOW_STYLE::default(),
            0,
            0,
            0,
            0,
            None,
            None,
            Some(instance),
            None,
        ) else {
            return;
        };
        SHELLHOOK.store(RegisterWindowMessageW(windows::core::w!("SHELLHOOK")), Ordering::Relaxed);
        if !RegisterShellHookWindow(hwnd).as_bool() {
            return;
        }
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, Some(hwnd), 0, 0).as_bool() {
            DispatchMessageW(&msg);
        }
    });
}

unsafe extern "system" fn shell_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let shellhook = SHELLHOOK.load(Ordering::Relaxed);
    if shellhook != 0 && msg == shellhook {
        let target = lparam.0;
        match wparam.0 {
            HSHELL_FLASH => flash(target),
            HSHELL_WINDOWACTIVATED | HSHELL_RUDEAPPACTIVATED => seen(target),
            HSHELL_WINDOWDESTROYED => closed(target),
            HSHELL_REDRAW => title_changed(target),
            _ => {}
        }
        return LRESULT(0);
    }
    windows::Win32::UI::WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam)
}

fn hwnd(raw: isize) -> HWND {
    HWND(raw as *mut _)
}

fn root_owner(raw: isize) -> isize {
    unsafe { GetAncestor(hwnd(raw), GA_ROOTOWNER) }.0 as isize
}

fn emit(list: Vec<isize>) {
    if let Some(app) = APP.get() {
        let _ = app.emit("app-attention", list);
    }
}

fn flash(raw: isize) {
    if raw == 0 || unsafe { GetForegroundWindow() }.0 as isize == raw {
        return;
    }
    // A dialog can flash on its own; the dock shows the main window it belongs to.
    let root = root_owner(raw);
    let Ok(mut list) = FLASHING.lock() else { return };
    let before = list.len();
    for h in [raw, root] {
        if h != 0 && !list.contains(&h) {
            list.push(h);
        }
    }
    if list.len() != before {
        emit(list.clone());
    }
}

fn seen(raw: isize) {
    if raw == 0 {
        return;
    }
    let root = root_owner(raw);
    let Ok(mut list) = FLASHING.lock() else { return };
    let before = list.len();
    list.retain(|&h| {
        h != raw && (root == 0 || (h != root && root_owner(h) != root)) && unsafe { IsWindow(Some(hwnd(h))) }.as_bool()
    });
    if list.len() != before {
        emit(list.clone());
    }
}

fn closed(raw: isize) {
    if let Some(counts) = COUNTS.get() {
        if let Ok(mut counts) = counts.lock() {
            counts.remove(&raw);
        }
    }
    let Ok(mut list) = FLASHING.lock() else { return };
    let before = list.len();
    list.retain(|&h| h != raw && unsafe { IsWindow(Some(hwnd(h))) }.as_bool());
    if list.len() != before {
        emit(list.clone());
    }
}

/// "(3) Inbox", "(12+) WhatsApp" or "Telegram (5)". The dock decides which ones it trusts; this
/// only notices when the number changes.
fn title_count(title: &str) -> Option<u32> {
    let title = title.trim();
    let digits = |s: &str| s.trim_end_matches('+').parse::<u32>().ok();
    if let Some(rest) = title.strip_prefix('(') {
        if let Some(n) = rest.split_once(')').and_then(|(n, _)| digits(n)) {
            return Some(n);
        }
    }
    let inner = title.strip_suffix(')')?;
    digits(&inner[inner.rfind('(')? + 1..])
}

fn title_changed(raw: isize) {
    if raw == 0 {
        return;
    }
    let mut buf = [0u16; 256];
    let len = unsafe { GetWindowTextW(hwnd(raw), &mut buf) }.max(0) as usize;
    let count = title_count(&String::from_utf16_lossy(&buf[..len]));
    let counts = COUNTS.get_or_init(|| Mutex::new(HashMap::new()));
    let Ok(mut counts) = counts.lock() else { return };
    let previous = counts.insert(raw, count);
    // Titles change all the time; the dock only needs to look again when a count appears,
    // changes or goes away.
    if previous.flatten() != count {
        if let Some(app) = APP.get() {
            let _ = app.emit("windows-changed", ());
        }
    }
}

/// Windows that asked for attention and haven't been looked at yet.
#[tauri::command]
pub fn get_attention_windows() -> Vec<isize> {
    let Ok(mut list) = FLASHING.lock() else { return Vec::new() };
    list.retain(|&h| unsafe { IsWindow(Some(hwnd(h))) }.as_bool());
    list.clone()
}

#[cfg(test)]
mod tests {
    use super::title_count;

    #[test]
    fn counts_in_titles() {
        assert_eq!(title_count("(3) Inbox - Gmail"), Some(3));
        assert_eq!(title_count("(12+) WhatsApp"), Some(12));
        assert_eq!(title_count("Telegram (5)"), Some(5));
        assert_eq!(title_count("Telegram"), None);
        assert_eq!(title_count("Notes (draft)"), None);
        assert_eq!(title_count("(beta) App"), None);
        assert_eq!(title_count(""), None);
    }
}
