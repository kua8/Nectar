use std::sync::atomic::{AtomicU32, Ordering};
use tauri::{AppHandle, Emitter};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN, VK_LCONTROL,
    VK_LMENU, VK_LSHIFT, VK_LWIN,
};
use windows::Win32::UI::WindowsAndMessaging::{GetMessageW, PeekMessageW, PostThreadMessageW, MSG, PM_NOREMOVE, WM_APP, WM_HOTKEY, WM_USER};
use windows::Win32::Foundation::{LPARAM, WPARAM};

const NOTCH_HOTKEY_ID: i32 = 1;
const RELOAD: u32 = WM_APP + 1;

static THREAD_ID: AtomicU32 = AtomicU32::new(0);

fn apply(app: &AppHandle, registered: &mut bool) {
    unsafe {
        if *registered {
            let _ = UnregisterHotKey(None, NOTCH_HOTKEY_ID);
            *registered = false;
        }
        let Some(spec) = crate::utils::get_setting_str(app, "nectar-notch-hotkey").filter(|s| !s.trim().is_empty()) else { return };
        let Some((modifiers, key)) = crate::launchers::parse_shortcut(&spec) else {
            crate::diagnostics::log(&format!("hotkey: could not read \"{spec}\""));
            return;
        };
        let mut mods = MOD_NOREPEAT;
        for m in modifiers {
            mods |= match m {
                VK_LCONTROL => MOD_CONTROL,
                VK_LMENU => MOD_ALT,
                VK_LSHIFT => MOD_SHIFT,
                VK_LWIN => MOD_WIN,
                _ => HOT_KEY_MODIFIERS(0),
            };
        }
        match RegisterHotKey(None, NOTCH_HOTKEY_ID, mods, key.0 as u32) {
            Ok(()) => *registered = true,
            Err(e) => crate::diagnostics::log(&format!("hotkey: could not register \"{spec}\": {e}")),
        }
    }
}

pub fn start(app: AppHandle) {
    std::thread::spawn(move || unsafe {
        let mut msg = MSG::default();
        let _ = PeekMessageW(&mut msg, None, WM_USER, WM_USER, PM_NOREMOVE);
        THREAD_ID.store(GetCurrentThreadId(), Ordering::SeqCst);
        let mut registered = false;
        apply(&app, &mut registered);
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            match msg.message {
                WM_HOTKEY => {
                    let _ = app.emit("notch-hotkey-toggle", ());
                }
                RELOAD => apply(&app, &mut registered),
                _ => {}
            }
        }
    });
}

pub fn reload() {
    let id = THREAD_ID.load(Ordering::SeqCst);
    if id != 0 {
        unsafe {
            let _ = PostThreadMessageW(id, RELOAD, WPARAM(0), LPARAM(0));
        }
    }
}
