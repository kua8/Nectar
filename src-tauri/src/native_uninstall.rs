
use std::os::windows::ffi::OsStrExt;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
use windows::Win32::System::Registry::{
    RegDeleteKeyValueW, RegDeleteTreeW, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE,
};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::{
    MessageBoxW, IDYES, MB_ICONINFORMATION, MB_ICONQUESTION, MB_OK, MB_TOPMOST, MB_YESNO, SW_SHOWNORMAL,
};
use windows::core::PCWSTR;

use crate::uninstall_registry::{
    desktop_dir, legacy_start_menu_dir, start_menu_programs_dir, wide, UNINSTALL_SUBKEY,
};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const TITLE: &str = "Nectar";

pub fn requested() -> bool {
    let by_arg = std::env::args().skip(1).any(|a| a == "--uninstall");
    let by_name = std::env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase()))
        .is_some_and(|n| n == "uninstall.exe");
    by_arg || by_name
}

fn message(text: &str, flags: windows::Win32::UI::WindowsAndMessaging::MESSAGEBOX_STYLE) -> windows::Win32::UI::WindowsAndMessaging::MESSAGEBOX_RESULT {
    let text_w = wide(text);
    let title_w = wide(TITLE);
    unsafe { MessageBoxW(None, PCWSTR(text_w.as_ptr()), PCWSTR(title_w.as_ptr()), flags | MB_TOPMOST) }
}

fn is_elevated() -> bool {
    unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION::default();
        let mut size = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut _),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut size,
        )
        .is_ok();
        let _ = CloseHandle(token);
        ok && elevation.TokenIsElevated != 0
    }
}

fn relaunch_elevated(exe: &Path) -> bool {
    let exe_w: Vec<u16> = exe.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    let verb_w = wide("runas");
    let args_w = wide("--uninstall");
    let result = unsafe {
        ShellExecuteW(None, PCWSTR(verb_w.as_ptr()), PCWSTR(exe_w.as_ptr()), PCWSTR(args_w.as_ptr()), None, SW_SHOWNORMAL)
    };
    (result.0 as isize) > 32
}

fn run_hidden(program: &str, args: &[&str]) -> Option<String> {
    std::process::Command::new(program)
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
}

fn stop_running_nectar() {
    let not_self = format!("PID ne {}", std::process::id());
    let filters = ["/FI", "IMAGENAME eq nectar.exe", "/FI", not_self.as_str()];

    let still_running = || {
        run_hidden("tasklist", &filters).is_some_and(|out| out.to_lowercase().contains("nectar.exe"))
    };

    let _ = run_hidden("taskkill", &filters);
    for _ in 0..16 {
        if !still_running() { return; }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }

    let mut forced = vec!["/F"];
    forced.extend_from_slice(&filters);
    let _ = run_hidden("taskkill", &forced);
    for _ in 0..8 {
        if !still_running() { return; }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
}

fn restore_taskbar_if_left_hidden() {
    let Ok(appdata) = std::env::var("APPDATA") else { return };
    let flag: PathBuf = Path::new(&appdata).join("com.kua8.nectar").join("taskbar_hidden.flag");
    if flag.exists() {
        crate::utils::set_taskbar_visibility(true, true);
        let _ = std::fs::remove_file(&flag);
    }
}

fn remove_shortcuts(all_users: bool) {
    let mut lnks = Vec::new();
    if let Some(d) = start_menu_programs_dir(all_users) { lnks.push(d.join("Nectar.lnk")); }
    if let Some(d) = legacy_start_menu_dir(all_users) { lnks.push(d.join("Nectar.lnk")); }
    if let Some(d) = desktop_dir(all_users) { lnks.push(d.join("Nectar.lnk")); }
    for lnk in lnks {
        let _ = std::fs::remove_file(lnk);
    }
}

fn remove_registry(all_users: bool) {
    unsafe {
        let root = if all_users { HKEY_LOCAL_MACHINE } else { HKEY_CURRENT_USER };
        let subkey_w = wide(UNINSTALL_SUBKEY);
        let _ = RegDeleteTreeW(root, PCWSTR(subkey_w.as_ptr()));

        let run_key = wide(r"Software\Microsoft\Windows\CurrentVersion\Run");
        let approved_key = wide(r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run");
        let value = wide("nectar");
        let _ = RegDeleteKeyValueW(HKEY_CURRENT_USER, PCWSTR(run_key.as_ptr()), PCWSTR(value.as_ptr()));
        let _ = RegDeleteKeyValueW(HKEY_CURRENT_USER, PCWSTR(approved_key.as_ptr()), PCWSTR(value.as_ptr()));
    }
}

pub fn run() {
    let Ok(exe) = std::env::current_exe() else { return };
    let Some(install_dir) = exe.parent().map(Path::to_path_buf) else { return };

    let scope = std::fs::read_to_string(install_dir.join(".install-scope")).unwrap_or_default();
    let all_users = scope.trim() == "all-users";

    if all_users && !is_elevated() {
        if !relaunch_elevated(&exe) {
            message("Nectar was installed for all users, so uninstalling it needs administrator permission.", MB_OK | MB_ICONINFORMATION);
        }
        return;
    }

    if message(
        "Uninstall Nectar?\n\nThis removes the app and its shortcuts. Your settings are kept in case you reinstall.",
        MB_YESNO | MB_ICONQUESTION,
    ) != IDYES
    {
        return;
    }

    stop_running_nectar();
    restore_taskbar_if_left_hidden();
    remove_shortcuts(all_users);
    remove_registry(all_users);

    let dir_str = install_dir.to_string_lossy().replace('\'', "''");
    let ps_command = format!(
        "Wait-Process -Id {pid} -ErrorAction SilentlyContinue; \
         for ($i = 0; $i -lt 40 -and (Test-Path -LiteralPath '{dir}'); $i++) {{ \
           Remove-Item -LiteralPath '{dir}' -Recurse -Force -ErrorAction SilentlyContinue -ErrorVariable failed; \
           if (Test-Path -LiteralPath '{dir}') {{ Start-Sleep -Milliseconds 500 }} \
         }}; \
         if (Test-Path -LiteralPath '{dir}') {{ Add-Content (Join-Path $env:TEMP 'nectar-uninstall.log') (\"Could not remove {dir}: \" + ($failed | Out-String)) }}",
        pid = std::process::id(),
        dir = dir_str
    );
    let spawn = |flags: u32| {
        std::process::Command::new("powershell")
            .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &ps_command])
            .creation_flags(flags)
            .spawn()
    };
    let spawned = spawn(CREATE_NO_WINDOW | 0x0100_0000).or_else(|_| spawn(CREATE_NO_WINDOW));

    if spawned.is_err() {
        message(
            &format!("Nectar was unregistered, but its folder couldn't be removed automatically. You can delete it yourself:\n\n{}", install_dir.display()),
            MB_OK | MB_ICONINFORMATION,
        );
        return;
    }
    message("Nectar has been uninstalled.", MB_OK | MB_ICONINFORMATION);
}
