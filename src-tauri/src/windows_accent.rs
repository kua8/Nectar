use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};
use windows::core::PCWSTR;
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY,
    HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE, REG_BINARY, REG_DWORD, REG_OPTION_NON_VOLATILE,
};
use windows::Win32::UI::WindowsAndMessaging::{SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE};

const DWM: &str = r"Software\Microsoft\Windows\DWM";
const ACCENT: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\Accent";
const VALUES: &[(&str, &str, bool)] = &[
    (DWM, "AccentColor", false),
    (DWM, "ColorizationColor", false),
    (DWM, "ColorizationAfterglow", false),
    (ACCENT, "AccentColorMenu", false),
    (ACCENT, "StartColorMenu", false),
    (ACCENT, "AccentPalette", true),
];

#[derive(Serialize, Deserialize, Clone)]
#[serde(tag = "type", content = "value")]
enum Saved {
    Dword(u32),
    Binary(Vec<u8>),
    Missing,
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn backup_path(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("windows-accent-backup.json"))
}

fn read_value(key: &str, name: &str, binary: bool) -> Saved {
    unsafe {
        let key_w = wide(key);
        let name_w = wide(name);
        let mut hkey = HKEY::default();
        if RegOpenKeyExW(HKEY_CURRENT_USER, PCWSTR(key_w.as_ptr()), Some(0), KEY_READ, &mut hkey).is_err() {
            return Saved::Missing;
        }
        let mut buf = [0u8; 64];
        let mut size = buf.len() as u32;
        let mut kind = if binary { REG_BINARY } else { REG_DWORD };
        let ok = RegQueryValueExW(hkey, PCWSTR(name_w.as_ptr()), None, Some(&mut kind), Some(buf.as_mut_ptr()), Some(&mut size)).is_ok();
        let _ = RegCloseKey(hkey);
        if !ok {
            return Saved::Missing;
        }
        if binary {
            Saved::Binary(buf[..size as usize].to_vec())
        } else {
            Saved::Dword(u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]))
        }
    }
}

fn write_value(key: &str, name: &str, value: &Saved) -> bool {
    unsafe {
        let key_w = wide(key);
        let name_w = wide(name);
        let mut hkey = HKEY::default();
        if RegCreateKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(key_w.as_ptr()),
            Some(0),
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            None,
            &mut hkey,
            None,
        )
        .is_err()
        {
            return false;
        }
        let ok = match value {
            Saved::Dword(v) => RegSetValueExW(hkey, PCWSTR(name_w.as_ptr()), Some(0), REG_DWORD, Some(&v.to_le_bytes())).is_ok(),
            Saved::Binary(b) => RegSetValueExW(hkey, PCWSTR(name_w.as_ptr()), Some(0), REG_BINARY, Some(b)).is_ok(),
            Saved::Missing => {
                let _ = RegDeleteValueW(hkey, PCWSTR(name_w.as_ptr()));
                true
            }
        };
        let _ = RegCloseKey(hkey);
        ok
    }
}

fn broadcast() {
    let setting = wide("ImmersiveColorSet");
    unsafe {
        let mut result = 0usize;
        let _ = SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            windows::Win32::Foundation::WPARAM(0),
            windows::Win32::Foundation::LPARAM(setting.as_ptr() as isize),
            SMTO_ABORTIFHUNG,
            200,
            Some(&mut result),
        );
    }
}

fn parse_hex(hex: &str) -> Option<[u8; 3]> {
    let h = hex.trim().trim_start_matches('#');
    if h.len() != 6 {
        return None;
    }
    let n = u32::from_str_radix(h, 16).ok()?;
    Some([(n >> 16) as u8, (n >> 8) as u8, n as u8])
}

fn mix(c: [u8; 3], to: u8, t: f32) -> [u8; 3] {
    let f = |v: u8| (v as f32 + (to as f32 - v as f32) * t).round().clamp(0.0, 255.0) as u8;
    [f(c[0]), f(c[1]), f(c[2])]
}

fn abgr(c: [u8; 3]) -> u32 {
    0xFF00_0000 | (c[2] as u32) << 16 | (c[1] as u32) << 8 | c[0] as u32
}

fn argb(c: [u8; 3]) -> u32 {
    0xC400_0000 | (c[0] as u32) << 16 | (c[1] as u32) << 8 | c[2] as u32
}

pub fn has_backup(app: &AppHandle) -> bool {
    backup_path(app).is_some_and(|p| p.is_file())
}

pub fn sync(app: &AppHandle, hex: &str) -> Result<(), String> {
    let base = parse_hex(hex).ok_or("Invalid color")?;
    let path = backup_path(app).ok_or("No settings folder")?;
    if !path.is_file() {
        let saved: BTreeMap<String, Saved> = VALUES
            .iter()
            .map(|(k, n, binary)| (format!("{k}|{n}"), read_value(k, n, *binary)))
            .collect();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let json = serde_json::to_string_pretty(&saved).map_err(|e| e.to_string())?;
        std::fs::write(&path, json).map_err(|e| e.to_string())?;
    }

    let swatches = [
        mix(base, 255, 0.6),
        mix(base, 255, 0.4),
        mix(base, 255, 0.2),
        base,
        mix(base, 0, 0.2),
        mix(base, 0, 0.4),
        mix(base, 0, 0.6),
        mix(base, 0, 0.8),
    ];
    let mut palette = Vec::with_capacity(32);
    for c in swatches {
        palette.extend_from_slice(&[c[0], c[1], c[2], 0xFF]);
    }
    let dark = mix(base, 0, 0.25);
    let new_values = [
        Saved::Dword(abgr(base)),
        Saved::Dword(argb(base)),
        Saved::Dword(argb(base)),
        Saved::Dword(abgr(base)),
        Saved::Dword(abgr(dark)),
        Saved::Binary(palette),
    ];
    for ((k, n, _), v) in VALUES.iter().zip(new_values.iter()) {
        write_value(k, n, v);
    }
    std::thread::spawn(broadcast);
    Ok(())
}

pub fn restore(app: &AppHandle) -> Result<(), String> {
    let path = backup_path(app).ok_or("No settings folder")?;
    let text = std::fs::read_to_string(&path).map_err(|_| "There is no saved Windows accent to restore".to_string())?;
    let saved: BTreeMap<String, Saved> = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    for (k, n, _) in VALUES {
        if let Some(v) = saved.get(&format!("{k}|{n}")) {
            write_value(k, n, v);
        }
    }
    let _ = std::fs::remove_file(&path);
    std::thread::spawn(broadcast);
    Ok(())
}
