use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;

fn nectar_running() -> bool {
    std::process::Command::new("tasklist")
        .args(["/FI", "IMAGENAME eq nectar.exe"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).contains("nectar.exe"))
}

fn stop_running_nectar() {
    let _ = std::process::Command::new("taskkill")
        .args(["/IM", "nectar.exe"])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    for _ in 0..12 {
        if !nectar_running() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }

    let _ = std::process::Command::new("taskkill")
        .args(["/F", "/T", "/IM", "nectar.exe"])
        .creation_flags(CREATE_NO_WINDOW)
        .output();
    for _ in 0..12 {
        if !nectar_running() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
}

fn run_once_key(all_users: bool) -> &'static str {
    if all_users {
        r"HKLM:\Software\Microsoft\Windows\CurrentVersion\RunOnce"
    } else {
        r"HKCU:\Software\Microsoft\Windows\CurrentVersion\RunOnce"
    }
}

// Files Nectar itself puts in the install folder (the payload is just nectar.exe)
const APP_FILES: &[&str] = &["nectar.exe", "uninstall.exe", ".install-scope"];

// True when the folder belongs to Nectar alone and can be removed as a whole.
// The installer extracts straight into the picked folder, so it may be shared (e.g. D:\Apps).
fn is_dedicated_install_dir(dir: &Path) -> bool {
    let Some(name) = dir.file_name().map(|n| n.to_string_lossy().to_lowercase()) else {
        return false;
    };

    if !name.contains("nectar") {
        return false;
    }

    if dir.parent().is_none() {
        return false;
    }

    if let Ok(userprofile) = std::env::var("USERPROFILE") {
        let user_path = Path::new(&userprofile);
        if dir == user_path || dir == user_path.join("Desktop") || dir == user_path.join("Downloads") {
            return false;
        }
    }

    true
}

fn remove_app_files_command(install_dir: &Path, all_users: bool) -> String {
    let dir = install_dir.to_string_lossy().replace('\'', "''");
    let paths: Vec<String> = APP_FILES
        .iter()
        .map(|f| install_dir.join(f).to_string_lossy().replace('\'', "''"))
        .collect();
    let files = paths.iter().map(|p| format!("'{p}'")).collect::<Vec<_>>().join(", ");
    let del_args = paths.iter().map(|p| format!("\"{p}\"")).collect::<Vec<_>>().join(" ");
    format!(
        "Wait-Process -Id {pid} -ErrorAction SilentlyContinue; \
         $files = @({files}); \
         $left = {{ @($files | Where-Object {{ Test-Path -LiteralPath $_ }}) }}; \
         for ($i = 0; $i -lt 40 -and (& $left).Count; $i++) {{ \
           & $left | ForEach-Object {{ Remove-Item -LiteralPath $_ -Force -ErrorAction SilentlyContinue -ErrorVariable +failed }}; \
           if ((& $left).Count) {{ Start-Sleep -Milliseconds 500 }} \
         }}; \
         if ((& $left).Count) {{ \
           Add-Content (Join-Path $env:TEMP 'nectar-uninstall.log') (\"Could not remove Nectar files from {dir}: \" + ($failed | Out-String)); \
           New-ItemProperty -Path '{run_once}' -Name 'NectarCleanup' -Value 'cmd /c del /f /q {del_args}' -Force -ErrorAction SilentlyContinue | Out-Null \
         }}",
        pid = std::process::id(),
        run_once = run_once_key(all_users)
    )
}

pub fn uninstall(install_dir: &Path, all_users: bool, shortcuts: &[PathBuf]) -> Result<(), String> {
    stop_running_nectar();

    for s in shortcuts {
        let _ = std::fs::remove_file(s);
    }

    let _ = crate::registry::remove_uninstall_entry(all_users);

    let pid = std::process::id();
    let dir_str = install_dir.to_string_lossy().replace('\'', "''");
    let ps_command = if !is_dedicated_install_dir(install_dir) {
        // Shared folder: remove only Nectar's own files, never the folder
        remove_app_files_command(install_dir, all_users)
    } else {
        format!(
            "Wait-Process -Id {pid} -ErrorAction SilentlyContinue; \
             for ($i = 0; $i -lt 40 -and (Test-Path -LiteralPath '{dir}'); $i++) {{ \
               Remove-Item -LiteralPath '{dir}' -Recurse -Force -ErrorAction SilentlyContinue -ErrorVariable failed; \
               if (Test-Path -LiteralPath '{dir}') {{ Start-Sleep -Milliseconds 500 }} \
             }}; \
             if (Test-Path -LiteralPath '{dir}') {{ \
               Add-Content (Join-Path $env:TEMP 'nectar-uninstall.log') (\"Could not remove {dir}: \" + ($failed | Out-String)); \
               New-ItemProperty -Path '{run_once}' -Name 'NectarCleanup' -Value 'cmd /c rd /s /q \"{dir}\"' -Force -ErrorAction SilentlyContinue | Out-Null \
             }}",
            pid = pid,
            dir = dir_str,
            run_once = run_once_key(all_users)
        )
    };

    let spawn = |flags: u32| {
        std::process::Command::new("powershell")
            .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &ps_command])
            .current_dir(std::env::temp_dir())
            .creation_flags(flags)
            .spawn()
    };
    spawn(CREATE_NO_WINDOW | CREATE_BREAKAWAY_FROM_JOB)
        .or_else(|_| spawn(CREATE_NO_WINDOW))
        .map_err(|e| e.to_string())?;

    Ok(())
}
