use std::os::windows::ffi::OsStrExt;
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::PCWSTR;

pub fn is_elevated() -> bool {
    unsafe {
        let mut token = windows::Win32::Foundation::HANDLE::default();
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

fn wide(s: &std::ffi::OsStr) -> Vec<u16> {
    s.encode_wide().chain(std::iter::once(0)).collect()
}

pub fn relaunch_elevated(args: &[String]) -> windows::core::Result<()> {
    unsafe {
        let exe = std::env::current_exe().map_err(|_| windows::core::Error::from_win32())?;
        let exe_w = wide(exe.as_os_str());
        let args_joined = args.join(" ");
        let args_w = wide(std::ffi::OsStr::new(&args_joined));
        let verb_w = wide(std::ffi::OsStr::new("runas"));

        let result = ShellExecuteW(
            None,
            PCWSTR(verb_w.as_ptr()),
            PCWSTR(exe_w.as_ptr()),
            PCWSTR(args_w.as_ptr()),
            None,
            SW_SHOWNORMAL,
        );

        if (result.0 as isize) <= 32 {
            return Err(windows::core::Error::from_win32());
        }
        Ok(())
    }
}
