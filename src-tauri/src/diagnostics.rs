use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Mutex, OnceLock};
use tauri::{AppHandle, Manager};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const MAX_LOG_BYTES: u64 = 1_000_000;
const LOG_FILE: &str = "nectar.log";
const REPORT_FILE: &str = "diagnostics.txt";
const HITBOX_INTERVAL_MS: i64 = 250;

static LOG_DIR: OnceLock<PathBuf> = OnceLock::new();
static LOG_LOCK: Mutex<()> = Mutex::new(());
static HITBOX_LOGGING: AtomicBool = AtomicBool::new(false);
static LAST_HITBOX_MS: [AtomicI64; 2] = [AtomicI64::new(0), AtomicI64::new(0)];

pub static HOOK_EVENTS: AtomicI64 = AtomicI64::new(0);
pub static HOOK_LAST_EVENT_MS: AtomicI64 = AtomicI64::new(0);

pub fn init() {
    let Ok(appdata) = std::env::var("APPDATA") else { return };
    let dir = PathBuf::from(appdata).join("com.kua8.nectar").join("logs");
    if fs::create_dir_all(&dir).is_err() {
        return;
    }
    let _ = LOG_DIR.set(dir);
    install_panic_hook();
    log(&format!(
        "---- Nectar {} starting (pid {}) ----",
        env!("CARGO_PKG_VERSION"),
        std::process::id()
    ));
}

pub fn log_dir() -> Option<PathBuf> {
    LOG_DIR.get().cloned()
}

pub fn log(msg: &str) {
    let Some(dir) = LOG_DIR.get() else { return };
    let _guard = LOG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let path = dir.join(LOG_FILE);
    if fs::metadata(&path).map(|m| m.len() > MAX_LOG_BYTES).unwrap_or(false) {
        let _ = fs::rename(&path, dir.join("nectar.old.log"));
    }
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&path) {
        let stamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
        let _ = writeln!(f, "[{stamp}] {msg}");
    }
}

fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current();
        let name = thread.name().unwrap_or("<unnamed>");
        let backtrace = std::backtrace::Backtrace::force_capture();
        log(&format!("PANIC on thread '{name}': {info}\n{backtrace}"));
        default_hook(info);
    }));
}

pub fn set_hitbox_logging(enabled: bool) {
    HITBOX_LOGGING.store(enabled, Ordering::Relaxed);
    log(&format!("hit-test logging {}", if enabled { "enabled" } else { "disabled" }));
}

pub fn hitbox_logging() -> bool {
    HITBOX_LOGGING.load(Ordering::Relaxed)
}

pub fn hitbox_log(slot: usize, now_ms: i64, line: impl FnOnce() -> String) {
    if !hitbox_logging() {
        return;
    }
    if now_ms - LAST_HITBOX_MS[slot].load(Ordering::Relaxed) < HITBOX_INTERVAL_MS {
        return;
    }
    LAST_HITBOX_MS[slot].store(now_ms, Ordering::Relaxed);
    log(&line());
}

fn run_hidden(program: &str, args: &[&str]) -> Option<String> {
    std::process::Command::new(program)
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
}

fn reg_value(key: &str, name: &str) -> Option<String> {
    let out = run_hidden("reg", &["query", key, "/v", name])?;
    out.lines()
        .find(|l| l.trim_start().starts_with(name))
        .and_then(|l| l.split_whitespace().last())
        .map(str::to_string)
}

fn windows_version() -> String {
    const KEY: &str = r"HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion";
    let name = reg_value(KEY, "ProductName").unwrap_or_default();
    let display = reg_value(KEY, "DisplayVersion").unwrap_or_default();
    let build = reg_value(KEY, "CurrentBuildNumber").unwrap_or_default();
    let ubr = reg_value(KEY, "UBR")
        .and_then(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).ok())
        .map(|v| v.to_string())
        .unwrap_or_default();
    format!("{name} {display} (build {build}.{ubr})")
}

fn gpu_names() -> String {
    run_hidden("powershell", &["-NoProfile", "-Command", "(Get-CimInstance Win32_VideoController).Name -join ', '"])
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

fn running_conflicts() -> String {
    const WATCH: [&str; 9] = [
        "razer", "rzsynapse", "synapse", "logi", "lghub", "autohotkey", "powertoys", "displayfusion", "actualtools",
    ];
    let Some(out) = run_hidden("tasklist", &["/FO", "CSV", "/NH"]) else { return String::new() };
    let mut found: Vec<String> = out
        .lines()
        .filter_map(|l| l.split(',').next())
        .map(|n| n.trim_matches('"').to_string())
        .filter(|n| {
            let low = n.to_lowercase();
            WATCH.iter().any(|w| low.contains(w))
        })
        .collect();
    found.sort();
    found.dedup();
    found.join(", ")
}

fn sensitive_key(key: &str) -> bool {
    let k = key.to_lowercase();
    ["caldav", "ics", "token", "password", "email", "account", "url", "location", "weather-cached"]
        .iter()
        .any(|s| k.contains(s))
}

fn settings_snapshot() -> String {
    let Some(cache) = crate::state::SETTINGS_CACHE.get() else { return "(settings not loaded)".into() };
    let Ok(guard) = cache.lock() else { return "(settings locked)".into() };
    let mut keys: Vec<&String> = guard.keys().filter(|k| !sensitive_key(k)).collect();
    keys.sort();
    keys.iter().map(|k| format!("  {k} = {}", guard[*k])).collect::<Vec<_>>().join("\n")
}

fn tail(max_bytes: usize) -> String {
    let Some(dir) = LOG_DIR.get() else { return String::new() };
    let Ok(text) = fs::read_to_string(dir.join(LOG_FILE)) else { return String::new() };
    if text.len() <= max_bytes {
        return text;
    }
    let mut start = text.len() - max_bytes;
    while !text.is_char_boundary(start) {
        start += 1;
    }
    text[start..].to_string()
}

fn window_report(app: &AppHandle) -> String {
    let mut out = String::new();
    for (label, win) in app.webview_windows() {
        if !matches!(label.as_str(), "main" | "dock" | "overlay") && !label.starts_with("main-") && !label.starts_with("dock-") {
            continue;
        }
        let pos = win.outer_position().map(|p| format!("{},{}", p.x, p.y)).unwrap_or_else(|_| "?".into());
        let size = win.outer_size().map(|s| format!("{}x{}", s.width, s.height)).unwrap_or_else(|_| "?".into());
        let scale = win.scale_factor().unwrap_or(0.0);
        let visible = win.is_visible().unwrap_or(false);
        out.push_str(&format!("  {label}: pos={pos} size={size} scale={scale} visible={visible}\n"));
    }
    if let Ok(m) = crate::state::dock_rects().lock() {
        for (label, r) in m.iter() {
            out.push_str(&format!("  reported dock rect [{label}]: x={} y={} w={} h={}\n", r.x, r.y, r.width, r.height));
        }
    }
    if let Ok(m) = crate::state::notch_rects().lock() {
        for (label, r) in m.iter() {
            out.push_str(&format!("  reported notch rect [{label}]: x={} y={} w={} h={}\n", r.x, r.y, r.width, r.height));
        }
    }
    out
}

fn monitor_report(app: &AppHandle) -> String {
    let Ok(monitors) = app.available_monitors() else { return "  (unavailable)\n".into() };
    monitors
        .iter()
        .map(|m| {
            let p = m.position();
            let s = m.size();
            format!("  {} {}x{} at {},{} scale={}\n", m.name().cloned().unwrap_or_default(), s.width, s.height, p.x, p.y, m.scale_factor())
        })
        .collect()
}

pub fn build_report(app: &AppHandle) -> String {
    let exe = std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_default();
    let now = crate::services::now_ms();
    let last = HOOK_LAST_EVENT_MS.load(Ordering::Relaxed);
    let hook_age = if last == 0 { "never".to_string() } else { format!("{}ms ago", now - last) };

    let report = format!(
        "Nectar diagnostics\n\
         ==================\n\
         Nectar: {version}\n\
         Windows: {windows}\n\
         WebView2: {webview}\n\
         GPU: {gpu}\n\
         Executable: {exe}\n\
         Elevated: {elevated}\n\
         Possibly conflicting apps running: {conflicts}\n\n\
         Monitors:\n{monitors}\n\
         Mouse hook: {events} events, last {hook_age}\n\n\
         Windows:\n{windows_info}\n\
         Settings:\n{settings}\n\n\
         Recent log:\n{log}\n",
        version = env!("CARGO_PKG_VERSION"),
        windows = windows_version(),
        webview = tauri::webview_version().unwrap_or_else(|_| "unknown".into()),
        gpu = gpu_names(),
        exe = exe,
        elevated = crate::native_uninstall::is_elevated(),
        conflicts = { let c = running_conflicts(); if c.is_empty() { "none detected".into() } else { c } },
        monitors = monitor_report(app),
        events = HOOK_EVENTS.load(Ordering::Relaxed),
        hook_age = hook_age,
        windows_info = window_report(app),
        settings = settings_snapshot(),
        log = tail(24_000),
    );

    if let Some(dir) = LOG_DIR.get() {
        let _ = fs::write(dir.join(REPORT_FILE), &report);
    }
    report
}
