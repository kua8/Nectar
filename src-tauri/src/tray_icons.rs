
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use windows::Win32::Foundation::{HWND, POINT, RECT};
use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationInvokePattern, TreeScope_Descendants,
    UIA_InvokePatternId,
};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowA, GetCursorPos, GetWindowLongA, GetWindowRect, IsWindowVisible, SetForegroundWindow,
    SetLayeredWindowAttributes, SetWindowLongA, SetWindowPos, ShowWindow, GWL_EXSTYLE, HWND_TOPMOST, LWA_ALPHA,
    SWP_NOSIZE, SWP_SHOWWINDOW, SW_HIDE, SW_SHOWNA, WS_EX_LAYERED, WS_EX_TRANSPARENT,
};
use windows::core::Interface;

static AUTOMATION_LOCK: Mutex<()> = Mutex::new(());
static FLYOUT_OPEN: AtomicBool = AtomicBool::new(false);

fn automation() -> windows::core::Result<IUIAutomation> {
    unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) }
}

fn find_window(class: &std::ffi::CStr) -> windows::core::Result<HWND> {
    unsafe { FindWindowA(windows::core::PCSTR(class.as_ptr() as *const u8), windows::core::PCSTR::null()) }
}

fn find_overflow_window() -> Option<HWND> {
    find_window(c"TopLevelWindowForOverflowXamlIsland")
        .or_else(|_| find_window(c"NotifyIconOverflowWindow"))
        .ok()
}

fn invoke_element(el: &IUIAutomationElement) -> bool {
    unsafe {
        let Ok(pattern) = el.GetCurrentPattern(UIA_InvokePatternId) else { return false };
        let Ok(invoke_pattern) = pattern.cast::<IUIAutomationInvokePattern>() else { return false };
        invoke_pattern.Invoke().is_ok()
    }
}

fn find_show_hidden_icons_button(automation: &IUIAutomation, tray_root: &IUIAutomationElement) -> Option<IUIAutomationElement> {
    unsafe {
        let condition = automation.CreateTrueCondition().ok()?;
        let all = tray_root.FindAll(TreeScope_Descendants, &condition).ok()?;
        for i in 0..all.Length().unwrap_or(0) {
            if let Ok(el) = all.GetElement(i) {
                let name = el.CurrentName().map(|s| s.to_string()).unwrap_or_default();
                if name.starts_with("Show Hidden Icons") {
                    return Some(el);
                }
            }
        }
        None
    }
}

fn place_flyout(flyout: HWND, anchor: Option<(i32, i32)>) {
    unsafe {
        let mut cursor = POINT::default();
        let _ = GetCursorPos(&mut cursor);
        let (ax, ay) = anchor.unwrap_or((cursor.x, cursor.y - 4));
        let point = POINT { x: ax, y: ay };
        let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        let _ = GetMonitorInfoW(MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST), &mut mi);
        let work = mi.rcWork;

        let mut rect = RECT::default();
        for _ in 0..10 {
            let _ = GetWindowRect(flyout, &mut rect);
            if rect.right - rect.left > 50 { break; }
            std::thread::sleep(std::time::Duration::from_millis(30));
        }
        let (w, h) = (rect.right - rect.left, rect.bottom - rect.top);

        let x = (ax - w / 2).clamp(work.left + 8, (work.right - w - 8).max(work.left + 8));
        let below = ay + 10;
        let y = if below + h <= work.bottom - 8 { below } else { (ay - h - 10).max(work.top + 8) };
        let _ = SetWindowPos(flyout, Some(HWND_TOPMOST), x, y, 0, 0, SWP_NOSIZE | SWP_SHOWWINDOW);
        let _ = SetForegroundWindow(flyout);
    }
}

pub fn open_native_tray(anchor: Option<(i32, i32)>) {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }

    if FLYOUT_OPEN.load(Ordering::SeqCst) {
        if let (Ok(auto), Ok(tray)) = (automation(), find_window(c"Shell_TrayWnd")) {
            if let Ok(root) = unsafe { auto.ElementFromHandle(tray) } {
                if let Some(button) = find_show_hidden_icons_button(&auto, &root) {
                    invoke_element(&button);
                }
            }
        }
        return;
    }
    let Ok(_guard) = AUTOMATION_LOCK.try_lock() else { return };

    let Ok(tray) = find_window(c"Shell_TrayWnd") else { return };
    let Ok(auto) = automation() else { return };

    let hidden = !unsafe { IsWindowVisible(tray) }.as_bool();
    let was_hidden_by_nectar = hidden && crate::state::NATIVE_TASKBAR_HIDDEN.swap(false, Ordering::SeqCst);
    let original_ex = unsafe { GetWindowLongA(tray, GWL_EXSTYLE) };
    if hidden {
        unsafe {
            SetWindowLongA(tray, GWL_EXSTYLE, original_ex | WS_EX_LAYERED.0 as i32 | WS_EX_TRANSPARENT.0 as i32);
            let _ = SetLayeredWindowAttributes(tray, windows::Win32::Foundation::COLORREF(0), 0, LWA_ALPHA);
            let _ = ShowWindow(tray, SW_SHOWNA);
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }

    let mut opened = false;
    if let Ok(root) = unsafe { auto.ElementFromHandle(tray) } {
        for _ in 0..12 {
            if let Some(button) = find_show_hidden_icons_button(&auto, &root) {
                opened = invoke_element(&button);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(150));
        }
    }

    if opened {
        FLYOUT_OPEN.store(true, Ordering::SeqCst);
        for _ in 0..40 {
            std::thread::sleep(std::time::Duration::from_millis(50));
            if let Some(flyout) = find_overflow_window() {
                place_flyout(flyout, anchor);
                break;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
        for _ in 0..2400 {
            if find_overflow_window().is_none_or(|w| !unsafe { IsWindowVisible(w) }.as_bool()) { break; }
            std::thread::sleep(std::time::Duration::from_millis(250));
        }
        FLYOUT_OPEN.store(false, Ordering::SeqCst);
    }

    if hidden {
        unsafe {
            let _ = ShowWindow(tray, SW_HIDE);
            SetWindowLongA(tray, GWL_EXSTYLE, original_ex);
        }
        if was_hidden_by_nectar {
            crate::utils::set_taskbar_visibility(false, false);
            crate::state::NATIVE_TASKBAR_HIDDEN.store(true, Ordering::SeqCst);
        }
    }
}
