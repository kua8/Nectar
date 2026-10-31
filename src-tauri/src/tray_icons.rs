
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
    FindWindowA, FindWindowExA, GetCursorPos, GetWindowLongA, GetWindowRect, IsWindowVisible, SetForegroundWindow,
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
                let automation_id = el.CurrentAutomationId().map(|s| s.to_string()).unwrap_or_default();
                if automation_id == "NotificationChevron" {
                    return Some(el);
                }
                let name = el.CurrentName().map(|s| s.to_string().to_lowercase()).unwrap_or_default();
                if name.contains("hidden icons") {
                    return Some(el);
                }
            }
        }
        None
    }
}

fn send_keys(keys: &[(windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY, bool)]) {
    use windows::Win32::UI::Input::KeyboardAndMouse::{SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP};
    let inputs: Vec<INPUT> = keys
        .iter()
        .map(|&(vk, up)| INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT { wVk: vk, wScan: 0, dwFlags: if up { KEYEVENTF_KEYUP } else { Default::default() }, time: 0, dwExtraInfo: 0 },
            },
        })
        .collect();
    unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
}

fn open_with_shortcut() -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::{VK_B, VK_LWIN, VK_RETURN};
    send_keys(&[(VK_LWIN, false), (VK_B, false), (VK_B, true), (VK_LWIN, true)]);
    std::thread::sleep(std::time::Duration::from_millis(350));
    send_keys(&[(VK_RETURN, false), (VK_RETURN, true)]);
    for _ in 0..40 {
        std::thread::sleep(std::time::Duration::from_millis(50));
        if find_overflow_window().is_some_and(|w| unsafe { IsWindowVisible(w) }.as_bool()) {
            return true;
        }
    }
    false
}

fn describe_elements(automation: &IUIAutomation, root: &IUIAutomationElement) -> String {
    unsafe {
        let Ok(condition) = automation.CreateTrueCondition() else { return "no condition".into() };
        let Ok(all) = root.FindAll(TreeScope_Descendants, &condition) else { return "FindAll failed".into() };
        let count = all.Length().unwrap_or(0);
        let mut parts = Vec::new();
        for i in 0..count.min(40) {
            if let Ok(el) = all.GetElement(i) {
                let id = el.CurrentAutomationId().map(|s| s.to_string()).unwrap_or_default();
                let name = el.CurrentName().map(|s| s.to_string()).unwrap_or_default();
                parts.push(format!("{id}|{name}"));
            }
        }
        format!("count={count} [{}]", parts.join("; "))
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

/// Loads UI Automation and walks the taskbar once, so the first real click isn't the slow one.
pub fn warm_up() {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
    let Ok(_guard) = AUTOMATION_LOCK.try_lock() else { return };
    let (Ok(tray), Ok(auto)) = (find_window(c"Shell_TrayWnd"), automation()) else { return };
    if let Ok(root) = unsafe { auto.ElementFromHandle(tray) } {
        let _ = find_show_hidden_icons_button(&auto, &root);
    }
}

pub fn open_native_tray(anchor: Option<(i32, i32)>) {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }

    crate::diagnostics::log(&format!("tray: open requested, anchor={anchor:?}, flyout_open={}", FLYOUT_OPEN.load(Ordering::SeqCst)));
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
    let Ok(_guard) = AUTOMATION_LOCK.try_lock() else {
        crate::diagnostics::log("tray: another tray action is still running");
        return;
    };

    let Ok(tray) = find_window(c"Shell_TrayWnd") else {
        crate::diagnostics::log("tray: Shell_TrayWnd not found");
        return;
    };
    let Ok(auto) = automation() else {
        crate::diagnostics::log("tray: UI Automation unavailable");
        return;
    };

    let hidden = !unsafe { IsWindowVisible(tray) }.as_bool();
    let was_hidden_by_nectar = hidden && crate::state::NATIVE_TASKBAR_HIDDEN.swap(false, Ordering::SeqCst);
    let original_ex = unsafe { GetWindowLongA(tray, GWL_EXSTYLE) };
    if hidden {
        unsafe {
            SetWindowLongA(tray, GWL_EXSTYLE, original_ex | WS_EX_LAYERED.0 as i32 | WS_EX_TRANSPARENT.0 as i32);
            let _ = SetLayeredWindowAttributes(tray, windows::Win32::Foundation::COLORREF(0), 0, LWA_ALPHA);
            let _ = ShowWindow(tray, SW_SHOWNA);
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
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

    if !opened {
        crate::diagnostics::log(&format!("tray: hidden icons button not found or not invokable (taskbar hidden={hidden})"));
        if let Ok(root) = unsafe { auto.ElementFromHandle(tray) } {
            crate::diagnostics::log(&format!("tray: taskbar elements: {}", describe_elements(&auto, &root)));
        }
        opened = open_with_shortcut();
        crate::diagnostics::log(&format!("tray: Win+B fallback opened={opened}"));
    }
    if opened {
        FLYOUT_OPEN.store(true, Ordering::SeqCst);
        let mut placed = false;
        for _ in 0..130 {
            std::thread::sleep(std::time::Duration::from_millis(15));
            if let Some(flyout) = find_overflow_window() {
                place_flyout(flyout, anchor);
                placed = true;
                break;
            }
        }
        crate::diagnostics::log(&format!("tray: flyout opened, placed={placed}"));
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

fn all_taskbars() -> Vec<HWND> {
    let mut taskbars = Vec::new();
    if let Ok(primary) = find_window(c"Shell_TrayWnd") {
        taskbars.push(primary);
    }
    let mut previous = None;
    loop {
        let next = unsafe {
            FindWindowExA(None, previous, windows::core::PCSTR(c"Shell_SecondaryTrayWnd".as_ptr() as *const u8), windows::core::PCSTR::null())
        };
        match next {
            Ok(hwnd) if !hwnd.0.is_null() => {
                taskbars.push(hwnd);
                previous = Some(hwnd);
            }
            _ => break,
        }
    }
    taskbars
}

pub fn schedule_taskbar_layout_refresh() {
    static SCHEDULED: AtomicBool = AtomicBool::new(false);
    if SCHEDULED.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_millis(2500));
        let Ok(_guard) = AUTOMATION_LOCK.try_lock() else { return };
        if !crate::state::NATIVE_TASKBAR_HIDDEN.swap(false, Ordering::SeqCst) {
            return;
        }

        let taskbars = all_taskbars();
        let originals: Vec<(HWND, i32)> = taskbars
            .iter()
            .map(|&hwnd| (hwnd, unsafe { GetWindowLongA(hwnd, GWL_EXSTYLE) }))
            .collect();
        for &(hwnd, ex) in &originals {
            unsafe {
                SetWindowLongA(hwnd, GWL_EXSTYLE, ex | WS_EX_LAYERED.0 as i32 | WS_EX_TRANSPARENT.0 as i32);
                let _ = SetLayeredWindowAttributes(hwnd, windows::Win32::Foundation::COLORREF(0), 0, LWA_ALPHA);
                let _ = ShowWindow(hwnd, SW_SHOWNA);
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
        for &(hwnd, ex) in &originals {
            unsafe {
                let _ = ShowWindow(hwnd, SW_HIDE);
                SetWindowLongA(hwnd, GWL_EXSTYLE, ex);
            }
        }
        crate::utils::set_taskbar_visibility(false, false);
        crate::state::NATIVE_TASKBAR_HIDDEN.store(true, Ordering::SeqCst);
    });
}

