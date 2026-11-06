use std::os::windows::process::CommandExt;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;
use windows::Win32::System::Threading::{
    GetCurrentProcess, SetPriorityClass, ABOVE_NORMAL_PRIORITY_CLASS, NORMAL_PRIORITY_CLASS,
    PROCESS_CREATION_FLAGS,
};

const TASK_NAME: &str = "Nectar";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const BOOST_LIMIT: Duration = Duration::from_secs(20);

static START: OnceLock<Instant> = OnceLock::new();
static SETTLED: AtomicBool = AtomicBool::new(false);

fn set_priority(class: PROCESS_CREATION_FLAGS) {
    unsafe {
        let _ = SetPriorityClass(GetCurrentProcess(), class);
    }
}

pub fn begin() {
    let _ = START.set(Instant::now());
    set_priority(ABOVE_NORMAL_PRIORITY_CLASS);
    std::thread::spawn(|| {
        std::thread::sleep(BOOST_LIMIT);
        settle("timeout");
    });
}

pub fn mark(label: &str) {
    if let Some(start) = START.get() {
        crate::diagnostics::log(&format!("startup: {label} after {} ms", start.elapsed().as_millis()));
    }
}

pub fn settle(reason: &str) {
    if SETTLED.swap(true, Ordering::Relaxed) {
        return;
    }
    set_priority(NORMAL_PRIORITY_CLASS);
    mark(&format!("normal priority ({reason})"));
}

fn schtasks(args: &[&str]) -> bool {
    Command::new("schtasks")
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn task_exists() -> bool {
    schtasks(&["/query", "/tn", TASK_NAME])
}

fn create_task() -> bool {
    let Ok(exe) = std::env::current_exe() else { return false };
    let user = format!(
        "{}\\{}",
        std::env::var("USERDOMAIN").unwrap_or_default(),
        std::env::var("USERNAME").unwrap_or_default()
    );
    let esc = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <Triggers><LogonTrigger><Enabled>true</Enabled><UserId>{user}</UserId></LogonTrigger></Triggers>
  <Principals><Principal id="A"><UserId>{user}</UserId><LogonType>InteractiveToken</LogonType><RunLevel>LeastPrivilege</RunLevel></Principal></Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Priority>4</Priority>
  </Settings>
  <Actions Context="A"><Exec><Command>{exe}</Command></Exec></Actions>
</Task>"#,
        user = esc(&user),
        exe = esc(&exe.to_string_lossy())
    );
    let mut bytes = vec![0xFF, 0xFE];
    for unit in xml.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    let path = std::env::temp_dir().join("nectar-logon-task.xml");
    if std::fs::write(&path, bytes).is_err() {
        return false;
    }
    let ok = schtasks(&["/create", "/tn", TASK_NAME, "/xml", &path.to_string_lossy(), "/f"]);
    let _ = std::fs::remove_file(&path);
    ok
}

fn delete_task() {
    if task_exists() {
        schtasks(&["/delete", "/tn", TASK_NAME, "/f"]);
    }
}

pub fn is_enabled(app: &AppHandle) -> bool {
    task_exists() || app.autolaunch().is_enabled().unwrap_or(false)
}

pub fn set_enabled(app: &AppHandle, on: bool) -> Result<(), String> {
    let launcher = app.autolaunch();
    if on {
        if create_task() {
            let _ = launcher.disable();
        } else {
            launcher.enable().map_err(|e| e.to_string())?;
        }
    } else {
        delete_task();
        let _ = launcher.disable();
    }
    Ok(())
}

pub fn migrate(app: &AppHandle) {
    let launcher = app.autolaunch();
    if launcher.is_enabled().unwrap_or(false) && !task_exists() && create_task() {
        let _ = launcher.disable();
        crate::diagnostics::log("startup: moved autostart to a logon task");
    }
}

#[tauri::command]
pub fn get_autostart(app: AppHandle) -> bool {
    is_enabled(&app)
}

#[tauri::command]
pub fn set_autostart(app: AppHandle, enabled: bool) -> Result<(), String> {
    set_enabled(&app, enabled)
}
