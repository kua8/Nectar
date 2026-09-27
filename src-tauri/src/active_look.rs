use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{SC_MINIMIZE, WM_NCACTIVATE, WM_SYSCOMMAND};

unsafe extern "system" fn active_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM, _id: usize, _data: usize) -> LRESULT {
    if msg == WM_NCACTIVATE {
        return DefSubclassProc(hwnd, msg, WPARAM(1), lparam);
    }
    DefSubclassProc(hwnd, msg, wparam, lparam)
}

pub fn keep_active_look(hwnd: HWND) {
    unsafe {
        let _ = SetWindowSubclass(hwnd, Some(active_proc), 1, 0);
    }
}

unsafe extern "system" fn no_minimize_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM, _id: usize, _data: usize) -> LRESULT {
    if msg == WM_SYSCOMMAND && (wparam.0 as u32 & 0xFFF0) == SC_MINIMIZE {
        return LRESULT(0);
    }
    DefSubclassProc(hwnd, msg, wparam, lparam)
}

pub fn prevent_minimize(hwnd: HWND) {
    unsafe {
        let _ = SetWindowSubclass(hwnd, Some(no_minimize_proc), 2, 0);
    }
}
