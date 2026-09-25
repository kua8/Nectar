use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;

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

pub fn uninstall(install_dir: &Path, all_users: bool, shortcuts: &[PathBuf]) -> Result<(), String> {
    stop_running_nectar();

    for s in shortcuts {
        let _ = std::fs::remove_file(s);
    }

    let _ = crate::registry::remove_uninstall_entry(all_users);

    let pid = std::process::id();
    let dir_str = install_dir.to_string_lossy().replace('\'', "''");
    let ps_command = format!(
        "Wait-Process -Id {pid} -ErrorAction SilentlyContinue; \
         for ($i = 0; $i -lt 40 -and (Test-Path -LiteralPath '{dir}'); $i++) {{ \
           Remove-Item -LiteralPath '{dir}' -Recurse -Force -ErrorAction SilentlyContinue -ErrorVariable failed; \
           if (Test-Path -LiteralPath '{dir}') {{ Start-Sleep -Milliseconds 500 }} \
         }}; \
         if (Test-Path -LiteralPath '{dir}') {{ Add-Content (Join-Path $env:TEMP 'nectar-uninstall.log') (\"Could not remove {dir}: \" + ($failed | Out-String)) }}",
        pid = pid,
        dir = dir_str
    );

    let spawn = |flags: u32| {
        std::process::Command::new("powershell")
            .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &ps_command])
            .creation_flags(flags)
            .spawn()
    };
    spawn(CREATE_NO_WINDOW | CREATE_BREAKAWAY_FROM_JOB)
        .or_else(|_| spawn(CREATE_NO_WINDOW))
        .map_err(|e| e.to_string())?;

    Ok(())
}
