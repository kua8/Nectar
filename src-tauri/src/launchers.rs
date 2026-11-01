// What the dock's Search button opens (Settings › Dock › Search With).
//
// PowerToys' launchers wait on named events that its own hotkey handler signals, so setting the
// event opens them exactly like their shortcut does (event names from PowerToys'
// src/common/interop/shared_constants.h). Raycast for Windows has no way to be opened from
// another app yet, so it, and any other launcher, gets its keyboard shortcut pressed instead.

use windows::Win32::UI::Input::KeyboardAndMouse::*;

const CMDPAL_SHOW_EVENT: &str = "Local\\PowerToysCmdPal-ShowEvent-62336fcd-8611-4023-9b30-091a6af4cc5a";
const POWER_LAUNCHER_SHARED_EVENT: &str = "Local\\PowerToysRunInvokeEvent-30f26ad7-d36d-4c0e-ab02-68bb5ff3c4ab";
const RAYCAST_DEFAULT_SHORTCUT: &str = "Alt+Space";

/// Opens the configured launcher. When the chosen one can't be opened (a PowerToys launcher
/// that isn't running, or a shortcut that can't be read) it opens Windows Search instead and
/// returns why, so the dock can say so rather than seem to ignore the setting.
pub fn open_search(provider: &str, shortcut: Option<&str>) -> Option<&'static str> {
    // The click went to Nectar, so only Nectar may bring a window to the front right now. Hand
    // that on, or the launcher can open behind the active window or without keyboard focus.
    allow_foreground();
    let result = match provider {
        "cmdpal" => signal_event(CMDPAL_SHOW_EVENT).then_some(()).ok_or("Command Palette isn't running. Turn it on in PowerToys."),
        "ptrun" => signal_event(POWER_LAUNCHER_SHARED_EVENT).then_some(()).ok_or("PowerToys Run isn't running. Turn it on in PowerToys."),
        "raycast" => send_shortcut(shortcut.filter(|s| !s.trim().is_empty()).unwrap_or(RAYCAST_DEFAULT_SHORTCUT))
            .then_some(())
            .ok_or("Couldn't press Raycast's shortcut. Check it in Settings › Dock."),
        "shortcut" => shortcut
            .is_some_and(send_shortcut)
            .then_some(())
            .ok_or("Set your launcher's shortcut in Settings › Dock."),
        _ => {
            send_shortcut("Win+S");
            Ok(())
        }
    };
    match result {
        Ok(()) => None,
        Err(note) => {
            send_shortcut("Win+S");
            Some(note)
        }
    }
}

fn allow_foreground() {
    use windows::Win32::UI::WindowsAndMessaging::{AllowSetForegroundWindow, ASFW_ANY};
    unsafe {
        let _ = AllowSetForegroundWindow(ASFW_ANY);
    }
}

fn signal_event(name: &str) -> bool {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{OpenEventW, SetEvent, EVENT_MODIFY_STATE};
    let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let Ok(handle) = OpenEventW(EVENT_MODIFY_STATE, false, PCWSTR(wide.as_ptr())) else { return false };
        let ok = SetEvent(handle).is_ok();
        let _ = CloseHandle(handle);
        ok
    }
}

fn key_for(name: &str) -> Option<VIRTUAL_KEY> {
    let upper = name.trim().to_ascii_uppercase();
    let bytes = upper.as_bytes();
    if bytes.len() == 1 && (bytes[0].is_ascii_uppercase() || bytes[0].is_ascii_digit()) {
        return Some(VIRTUAL_KEY(bytes[0] as u16));
    }
    if let Some(n) = upper.strip_prefix('F').and_then(|n| n.parse::<u16>().ok()) {
        if (1..=24).contains(&n) {
            return Some(VIRTUAL_KEY(VK_F1.0 + n - 1));
        }
    }
    Some(match upper.as_str() {
        "SPACE" => VK_SPACE,
        "ENTER" | "RETURN" => VK_RETURN,
        "TAB" => VK_TAB,
        "ESC" | "ESCAPE" => VK_ESCAPE,
        "BACKSPACE" => VK_BACK,
        "DELETE" | "DEL" => VK_DELETE,
        "INSERT" | "INS" => VK_INSERT,
        "HOME" => VK_HOME,
        "END" => VK_END,
        "PAGEUP" => VK_PRIOR,
        "PAGEDOWN" => VK_NEXT,
        "UP" => VK_UP,
        "DOWN" => VK_DOWN,
        "LEFT" => VK_LEFT,
        "RIGHT" => VK_RIGHT,
        "`" => VK_OEM_3,
        "-" => VK_OEM_MINUS,
        "=" => VK_OEM_PLUS,
        "," => VK_OEM_COMMA,
        "." => VK_OEM_PERIOD,
        "/" => VK_OEM_2,
        ";" => VK_OEM_1,
        "'" => VK_OEM_7,
        "[" => VK_OEM_4,
        "]" => VK_OEM_6,
        "\\" => VK_OEM_5,
        _ => return None,
    })
}

/// Presses a shortcut written like "Alt+Space" or "Ctrl+Shift+K": modifiers (Ctrl, Alt, Shift,
/// Win) then exactly one key. Returns false if it can't be parsed.
pub fn send_shortcut(spec: &str) -> bool {
    let mut modifiers = Vec::new();
    let mut key = None;
    for part in spec.split('+').map(str::trim).filter(|p| !p.is_empty()) {
        match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => modifiers.push(VK_LCONTROL),
            "alt" => modifiers.push(VK_LMENU),
            "shift" => modifiers.push(VK_LSHIFT),
            "win" | "windows" | "meta" | "super" => modifiers.push(VK_LWIN),
            _ if key.is_none() => key = Some(key_for(part)),
            _ => return false,
        }
    }
    // "Ctrl++" style specs don't split cleanly; a trailing "+" means the plus key itself
    if key.is_none() && spec.trim_end().ends_with("++") {
        key = Some(Some(VK_OEM_PLUS));
    }
    let Some(Some(key)) = key else { return false };

    let input = |vk: VIRTUAL_KEY, up: bool| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT { wVk: vk, wScan: 0, dwFlags: if up { KEYEVENTF_KEYUP } else { Default::default() }, time: 0, dwExtraInfo: 0 },
        },
    };
    let mut inputs: Vec<INPUT> = modifiers.iter().map(|&m| input(m, false)).collect();
    inputs.push(input(key, false));
    inputs.push(input(key, true));
    inputs.extend(modifiers.iter().rev().map(|&m| input(m, true)));
    unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) == inputs.len() as u32 }
}
