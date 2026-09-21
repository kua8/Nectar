use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// If Nectar itself is still running (e.g. uninstall launched directly from Add/Remove
/// Programs rather than through Nectar's own "Uninstall" button), ask it to close
/// gracefully first. A plain kill would skip Nectar's own quit path, which is what
/// restores the native taskbar out of auto-hide and unregisters its appbars — leaving
/// those stuck if we didn't wait here.
fn stop_running_nectar() {
    let _ = std::process::Command::new("taskkill")
        .args(["/IM", "nectar.exe"])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    for _ in 0..20 {
        let still_running = std::process::Command::new("tasklist")
            .args(["/FI", "IMAGENAME eq nectar.exe"])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).contains("nectar.exe"));
        if !still_running {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
}

/// Removes shortcuts and the registry entry, then hands off directory deletion to a
/// detached PowerShell process — the running uninstall.exe can't delete its own file
/// while it's still executing, and the user may sit on the "done" screen for a while,
/// so rather than guessing a fixed delay, the detached process waits on our own PID
/// and only deletes once we've actually exited.
pub fn uninstall(install_dir: &Path, all_users: bool, shortcuts: &[PathBuf]) -> Result<(), String> {
    stop_running_nectar();

    for s in shortcuts {
        let _ = std::fs::remove_file(s);
    }

    let _ = crate::registry::remove_uninstall_entry(all_users);

    let pid = std::process::id();
    let dir_str = install_dir.to_string_lossy().replace('\'', "''");
    let ps_command = format!(
        "Wait-Process -Id {pid} -ErrorAction SilentlyContinue; Remove-Item -LiteralPath '{}' -Recurse -Force -ErrorAction SilentlyContinue",
        dir_str
    );
    std::process::Command::new("powershell")
        .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &ps_command])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| e.to_string())?;

    Ok(())
}
