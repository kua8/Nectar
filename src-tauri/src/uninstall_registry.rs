use windows::Win32::System::Registry::{
    RegCreateKeyExW, RegSetValueExW, RegCloseKey, HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE,
    KEY_WRITE, REG_DWORD, REG_OPTION_NON_VOLATILE, REG_SZ,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED,
    IPersistFile,
};
use windows::Win32::UI::Shell::{
    IShellLinkW, SHGetKnownFolderPath, ShellLink, FOLDERID_CommonPrograms, FOLDERID_CommonStartMenu,
    FOLDERID_Desktop, FOLDERID_Programs, FOLDERID_PublicDesktop, FOLDERID_StartMenu, KF_FLAG_CREATE,
};
use windows::core::{Interface, GUID, PCWSTR};
use std::path::{Path, PathBuf};

pub const UNINSTALL_SUBKEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\Nectar";

pub fn wide(s: &str) -> Vec<u16> {
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

fn known_folder(rfid: *const GUID) -> Option<PathBuf> {
    unsafe {
        let pwstr = SHGetKnownFolderPath(rfid, KF_FLAG_CREATE, None).ok()?;
        let path = pwstr.to_string().ok();
        CoTaskMemFree(Some(pwstr.0 as *const _));
        path.map(PathBuf::from)
    }
}

pub fn start_menu_programs_dir(all_users: bool) -> Option<PathBuf> {
    known_folder(if all_users { &FOLDERID_CommonPrograms } else { &FOLDERID_Programs })
}

pub fn legacy_start_menu_dir(all_users: bool) -> Option<PathBuf> {
    known_folder(if all_users { &FOLDERID_CommonStartMenu } else { &FOLDERID_StartMenu })
}

pub fn desktop_dir(all_users: bool) -> Option<PathBuf> {
    known_folder(if all_users { &FOLDERID_PublicDesktop } else { &FOLDERID_Desktop })
}

fn create_shortcut(lnk_path: &Path, target: &Path) -> windows::core::Result<()> {
    unsafe {
        let shell_link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_ALL)?;

        let target_w = wide(&target.to_string_lossy());
        shell_link.SetPath(PCWSTR(target_w.as_ptr()))?;

        if let Some(dir) = target.parent() {
            let dir_w = wide(&dir.to_string_lossy());
            shell_link.SetWorkingDirectory(PCWSTR(dir_w.as_ptr()))?;
        }

        shell_link.SetIconLocation(PCWSTR(target_w.as_ptr()), 0)?;

        let desc_w = wide("Nectar");
        shell_link.SetDescription(PCWSTR(desc_w.as_ptr()))?;

        let persist_file: IPersistFile = shell_link.cast()?;
        let lnk_w = wide(&lnk_path.to_string_lossy());
        persist_file.Save(PCWSTR(lnk_w.as_ptr()), true)?;
    }
    Ok(())
}

fn migrate_start_menu_shortcut(exe: &Path, all_users: bool) {
    let (Some(legacy_dir), Some(programs_dir)) =
        (legacy_start_menu_dir(all_users), start_menu_programs_dir(all_users))
    else {
        return;
    };
    let legacy = legacy_dir.join("Nectar.lnk");
    if !legacy.exists() {
        return;
    }

    let fixed = programs_dir.join("Nectar.lnk");
    if !fixed.exists() {
        let created = unsafe {
            let com = CoInitializeEx(None, COINIT_MULTITHREADED).is_ok();
            let res = create_shortcut(&fixed, exe);
            if com { CoUninitialize(); }
            res.is_ok()
        };
        if !created {
            return;
        }
    }
    let _ = std::fs::remove_file(&legacy);
}

fn installer_marker() -> Vec<u8> {
    let reversed: &[u8] = std::hint::black_box(b"ssergorp-llatsninu");
    reversed.iter().rev().copied().collect()
}

fn is_installer_binary(path: &Path) -> bool {
    let marker = installer_marker();
    let Ok(bytes) = std::fs::read(path) else { return false };
    bytes.windows(marker.len()).any(|w| w == marker.as_slice())
}

fn repair_uninstaller(exe: &Path, uninstall_exe: &Path) {
    let Ok(exe_meta) = std::fs::metadata(exe) else { return };
    if let Ok(meta) = std::fs::metadata(uninstall_exe) {
        if meta.len() == exe_meta.len() && meta.modified().ok() == exe_meta.modified().ok() {
            return;
        }
        if is_installer_binary(uninstall_exe) {
            return;
        }
    }
    let _ = std::fs::copy(exe, uninstall_exe);
}

pub fn heal_uninstall_registration() {
    let Ok(current_exe) = std::env::current_exe() else { return };
    let Some(install_dir) = current_exe.parent() else { return };

    let scope_file = install_dir.join(".install-scope");
    let Ok(scope) = std::fs::read_to_string(&scope_file) else { return };
    let all_users = scope.trim() == "all-users";

    let uninstall_exe = install_dir.join("uninstall.exe");
    repair_uninstaller(&current_exe, &uninstall_exe);
    migrate_start_menu_shortcut(&current_exe, all_users);

    let uninstall_string = format!("\"{}\" --uninstall", uninstall_exe.to_string_lossy());
    let install_location = install_dir.to_string_lossy().to_string();
    let icon_str = format!("{},0", current_exe.to_string_lossy());
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installer_marker_spells_the_event_name() {
        assert_eq!(String::from_utf8(installer_marker()).unwrap(), "uninstall-progress");
    }
}
