use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteTreeW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
    HKEY_LOCAL_MACHINE, KEY_WRITE, REG_DWORD, REG_OPTION_NON_VOLATILE, REG_SZ,
};
use windows::core::PCWSTR;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

const UNINSTALL_SUBKEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\Nectar";

pub struct UninstallInfo<'a> {
    pub display_name: &'a str,
    pub display_icon: &'a str,
    pub display_version: &'a str,
    pub publisher: &'a str,
    pub install_location: &'a str,
    pub uninstall_string: &'a str,
    pub estimated_size_kb: u32,
}

pub fn write_uninstall_entry(all_users: bool, info: &UninstallInfo) -> windows::core::Result<()> {
    unsafe {
        let root = if all_users { HKEY_LOCAL_MACHINE } else { HKEY_CURRENT_USER };
        let subkey_w = wide(UNINSTALL_SUBKEY);
        let mut hkey = HKEY::default();

        RegCreateKeyExW(
            root,
            PCWSTR(subkey_w.as_ptr()),
            Some(0),
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            None,
            &mut hkey,
            None,
        )
        .ok()?;

        set_string(hkey, "DisplayName", info.display_name)?;
        set_string(hkey, "DisplayIcon", info.display_icon)?;
        set_string(hkey, "DisplayVersion", info.display_version)?;
        set_string(hkey, "Publisher", info.publisher)?;
        set_string(hkey, "InstallLocation", info.install_location)?;
        set_string(hkey, "UninstallString", info.uninstall_string)?;
        set_dword(hkey, "NoModify", 1)?;
        set_dword(hkey, "NoRepair", 1)?;
        set_dword(hkey, "EstimatedSize", info.estimated_size_kb)?;

        RegCloseKey(hkey).ok()?;
    }
    Ok(())
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

pub fn remove_uninstall_entry(all_users: bool) -> windows::core::Result<()> {
    unsafe {
        let root = if all_users { HKEY_LOCAL_MACHINE } else { HKEY_CURRENT_USER };
        let subkey_w = wide(UNINSTALL_SUBKEY);
        RegDeleteTreeW(root, PCWSTR(subkey_w.as_ptr())).ok()
    }
}
