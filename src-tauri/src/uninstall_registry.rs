use windows::Win32::System::Registry::{
    RegCreateKeyExW, RegSetValueExW, RegCloseKey, HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE,
    KEY_WRITE, REG_DWORD, REG_OPTION_NON_VOLATILE, REG_SZ,
};
use windows::core::PCWSTR;

const UNINSTALL_SUBKEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\Nectar";

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

unsafe fn set_string(hkey: HKEY, name: &str, value: &str) -> windows::core::Result<()> {
    let name_w = wide(name);
    let value_w = wide(value);
    let bytes = std::slice::from_raw_parts(value_w.as_ptr() as *const u8, value_w.len() * 2);
    RegSetValueExW(hkey, PCWSTR(name_w.as_ptr()), Some(0), REG_SZ, Some(bytes)).ok()
}

unsafe fn set_dword(hkey: HKEY, name: &str, value: u32) -> windows::core::Result<()> {
    let name_w = wide(name);
    let bytes = value.to_le_bytes();
    RegSetValueExW(hkey, PCWSTR(name_w.as_ptr()), Some(0), REG_DWORD, Some(&bytes)).ok()
}

fn dir_size_kb(path: &std::path::Path) -> u32 {
    fn walk(dir: &std::path::Path, total: &mut u64) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, total);
            } else if let Ok(meta) = entry.metadata() {
                *total += meta.len();
            }
        }
    }
    let mut total = 0u64;
    walk(path, &mut total);
    (total / 1024) as u32
}

/// The custom installer's uninstaller (a copy of itself at `uninstall.exe`) and its
/// Add/Remove Programs entry both get silently overwritten by Tauri's bundled NSIS
/// installer whenever the auto-updater applies an update, since NSIS's own template
/// writes a generic uninstaller to that exact filename and its own registry values to
/// the same key. Re-asserting both on every startup heals that instead of requiring a
/// build-level fix to the NSIS template itself.
pub fn heal_uninstall_registration() {
    let Ok(current_exe) = std::env::current_exe() else { return };
    let Some(install_dir) = current_exe.parent() else { return };

    // Only present on installs made through the custom installer-app; absent for
    // portable/dev usage, where none of this applies.
    let scope_file = install_dir.join(".install-scope");
    let Ok(scope) = std::fs::read_to_string(&scope_file) else { return };
    let all_users = scope.trim() == "all-users";

    let uninstall_exe = install_dir.join("uninstall.exe");
    let _ = std::fs::copy(&current_exe, &uninstall_exe);

    let uninstall_string = format!("\"{}\" --uninstall", uninstall_exe.to_string_lossy());
    let install_location = install_dir.to_string_lossy().to_string();
    let icon_str = current_exe.to_string_lossy().to_string();
    let size_kb = dir_size_kb(install_dir);

    unsafe {
        let root = if all_users { HKEY_LOCAL_MACHINE } else { HKEY_CURRENT_USER };
        let subkey_w = wide(UNINSTALL_SUBKEY);
        let mut hkey = HKEY::default();

        let created = RegCreateKeyExW(
            root,
            PCWSTR(subkey_w.as_ptr()),
            Some(0),
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            None,
            &mut hkey,
            None,
        );
        if created.is_err() {
            return;
        }

        let _ = set_string(hkey, "DisplayName", "Nectar");
        let _ = set_string(hkey, "DisplayIcon", &icon_str);
        let _ = set_string(hkey, "DisplayVersion", env!("CARGO_PKG_VERSION"));
        let _ = set_string(hkey, "Publisher", "kua8");
        let _ = set_string(hkey, "InstallLocation", &install_location);
        let _ = set_string(hkey, "UninstallString", &uninstall_string);
        let _ = set_dword(hkey, "NoModify", 1);
        let _ = set_dword(hkey, "NoRepair", 1);
        let _ = set_dword(hkey, "EstimatedSize", size_kb);

        let _ = RegCloseKey(hkey);
    }
}
