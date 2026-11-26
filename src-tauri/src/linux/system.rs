//! System status and controls on Linux: PipeWire/PulseAudio volume, backlight,
//! battery, NetworkManager, BlueZ, and /proc based metrics.

use crate::utils::{run, run_ok, which};
use serde::Serialize;
use std::sync::Mutex;

// ---------------------------------------------------------------- volume

/// (volume 0..1, muted) of the default sink.
pub fn volume_state() -> Option<(f32, bool)> {
    if which("wpctl") {
        if let Some(out) = run("wpctl", &["get-volume", "@DEFAULT_AUDIO_SINK@"]) {
            // "Volume: 0.45" or "Volume: 0.45 [MUTED]"
            let vol = out.split_whitespace().nth(1).and_then(|v| v.parse::<f32>().ok())?;
            return Some((vol.clamp(0.0, 1.5), out.contains("MUTED")));
        }
    }
    let vol = run("pactl", &["get-sink-volume", "@DEFAULT_SINK@"])?;
    let pct = vol.split('/').nth(1)?.trim().trim_end_matches('%').trim().parse::<f32>().ok()?;
    let muted = run("pactl", &["get-sink-mute", "@DEFAULT_SINK@"]).is_some_and(|m| m.contains("yes"));
    Some((pct / 100.0, muted))
}

pub fn set_volume(v: f32) {
    let v = v.clamp(0.0, 1.0);
    if which("wpctl") {
        run_ok("wpctl", &["set-volume", "@DEFAULT_AUDIO_SINK@", &format!("{v:.3}")]);
        if v > 0.0 {
            run_ok("wpctl", &["set-mute", "@DEFAULT_AUDIO_SINK@", "0"]);
        }
    } else {
        run_ok("pactl", &["set-sink-volume", "@DEFAULT_SINK@", &format!("{}%", (v * 100.0).round())]);
    }
}

pub fn toggle_mute() {
    if which("wpctl") {
        run_ok("wpctl", &["set-mute", "@DEFAULT_AUDIO_SINK@", "toggle"]);
    } else {
        run_ok("pactl", &["set-sink-mute", "@DEFAULT_SINK@", "toggle"]);
    }
}

#[derive(Serialize, Clone)]
pub struct AudioDevice {
    id: String,
    name: String,
    is_default: bool,
}

pub fn audio_devices() -> Vec<AudioDevice> {
    let default = run("pactl", &["get-default-sink"]).unwrap_or_default();
    let Some(json) = run("pactl", &["-f", "json", "list", "sinks"]) else { return Vec::new() };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&json) else { return Vec::new() };
    v.as_array()
        .map(|sinks| {
            sinks
                .iter()
                .filter_map(|s| {
                    let id = s["name"].as_str()?.to_string();
                    let name = s["description"].as_str().unwrap_or(&id).to_string();
                    Some(AudioDevice { is_default: id == default, id, name })
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn set_default_audio_device(id: &str) -> Result<(), String> {
    if run_ok("pactl", &["set-default-sink", id]) { Ok(()) } else { Err("pactl set-default-sink failed".into()) }
}

#[derive(Clone)]
pub struct SinkInput {
    pub index: u32,
    pub pid: u32,
    pub name: String,
    pub binary: Option<String>,
    pub volume: f32,
    pub muted: bool,
}

pub fn sink_inputs() -> Vec<SinkInput> {
    let Some(json) = run("pactl", &["-f", "json", "list", "sink-inputs"]) else { return Vec::new() };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&json) else { return Vec::new() };
    let Some(arr) = v.as_array() else { return Vec::new() };
    arr.iter()
        .filter_map(|s| {
            let props = &s["properties"];
            let pid = props["application.process.id"].as_str().and_then(|p| p.parse().ok()).unwrap_or(0);
            let name = props["application.name"].as_str().or(props["media.name"].as_str()).unwrap_or("Unknown").to_string();
            let vol = s["volume"]
                .as_object()
                .and_then(|channels| {
                    let vals: Vec<f32> = channels
                        .values()
                        .filter_map(|c| c["value_percent"].as_str())
                        .filter_map(|p| p.trim_end_matches('%').trim().parse::<f32>().ok())
                        .collect();
                    (!vals.is_empty()).then(|| vals.iter().sum::<f32>() / vals.len() as f32 / 100.0)
                })
                .unwrap_or(1.0);
            Some(SinkInput {
                index: s["index"].as_u64()? as u32,
                pid,
                name,
                binary: props["application.process.binary"].as_str().map(String::from),
                volume: vol,
                muted: s["mute"].as_bool().unwrap_or(false),
            })
        })
        .collect()
}

pub fn set_sink_input_volume(pid: u32, volume: f32) -> Result<(), String> {
    let pct = format!("{}%", (volume.clamp(0.0, 1.0) * 100.0).round());
    let mut any = false;
    for s in sink_inputs().into_iter().filter(|s| s.pid == pid) {
        any |= run_ok("pactl", &["set-sink-input-volume", &s.index.to_string(), &pct]);
    }
    if any { Ok(()) } else { Err("No audio stream for that process".into()) }
}

pub fn set_sink_input_mute(pid: u32, muted: bool) -> Result<(), String> {
    let mut any = false;
    for s in sink_inputs().into_iter().filter(|s| s.pid == pid) {
        any |= run_ok("pactl", &["set-sink-input-mute", &s.index.to_string(), if muted { "1" } else { "0" }]);
    }
    if any { Ok(()) } else { Err("No audio stream for that process".into()) }
}

// ---------------------------------------------------------------- brightness

fn backlight_dir() -> Option<std::path::PathBuf> {
    let entries = std::fs::read_dir("/sys/class/backlight").ok()?;
    let mut dirs: Vec<_> = entries.flatten().map(|e| e.path()).collect();
    // Prefer firmware/platform interfaces over raw ones, like systemd does.
    dirs.sort_by_key(|d| {
        match std::fs::read_to_string(d.join("type")).unwrap_or_default().trim() {
            "firmware" => 0,
            "platform" => 1,
            _ => 2,
        }
    });
    dirs.into_iter().next()
}

fn read_u32(p: std::path::PathBuf) -> Option<u32> {
    std::fs::read_to_string(p).ok()?.trim().parse().ok()
}

static BRIGHTNESS_CACHE: Mutex<Option<u32>> = Mutex::new(None);

pub fn set_brightness_cache(v: Option<u32>) {
    if let Ok(mut g) = BRIGHTNESS_CACHE.lock() {
        *g = v;
    }
}

/// Last value seen by the brightness watcher; None on machines without a backlight.
pub fn get_brightness_cached() -> Option<u32> {
    BRIGHTNESS_CACHE.lock().ok().and_then(|g| *g)
}

pub fn get_brightness() -> Option<u32> {
    let dir = backlight_dir()?;
    let cur = read_u32(dir.join("brightness"))?;
    let max = read_u32(dir.join("max_brightness"))?.max(1);
    Some(((cur as f64 / max as f64) * 100.0).round() as u32)
}

/// Sets backlight percent through logind, which allows the active session without root.
pub fn set_brightness(percent: u32) -> bool {
    let Some(dir) = backlight_dir() else { return false };
    let Some(max) = read_u32(dir.join("max_brightness")) else { return false };
    let name = dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    // Never go fully dark from a slider.
    let value = ((percent.clamp(1, 100) as f64 / 100.0) * max as f64).round().max(1.0) as u32;
    if run_ok(
        "busctl",
        &["call", "org.freedesktop.login1", "/org/freedesktop/login1/session/auto",
          "org.freedesktop.login1.Session", "SetBrightness", "ssu", "backlight", &name, &value.to_string()],
    ) {
        return true;
    }
    if which("brightnessctl") {
        return run_ok("brightnessctl", &["set", &format!("{}%", percent.clamp(1, 100))]);
    }
    std::fs::write(dir.join("brightness"), value.to_string()).is_ok()
}

// ---------------------------------------------------------------- power

pub fn has_battery() -> bool {
    std::fs::read_dir("/sys/class/power_supply")
        .map(|d| {
            d.flatten().any(|e| {
                std::fs::read_to_string(e.path().join("type")).is_ok_and(|t| t.trim() == "Battery")
                    && std::fs::read_to_string(e.path().join("scope")).map(|s| s.trim() != "Device").unwrap_or(true)
            })
        })
        .unwrap_or(false)
}

/// Power saver is on when the power-profiles-daemon profile is "power-saver".
pub fn battery_saver() -> bool {
    run("powerprofilesctl", &["get"]).is_some_and(|p| p.trim() == "power-saver")
}

// ---------------------------------------------------------------- network

pub fn wifi_enabled() -> bool {
    run("nmcli", &["radio", "wifi"]).is_some_and(|s| s.trim() == "enabled")
}

pub fn wifi_connected() -> bool {
    run("nmcli", &["-t", "-f", "TYPE,STATE", "device"])
        .is_some_and(|s| s.lines().any(|l| l == "wifi:connected"))
}

pub fn set_wifi(enabled: bool) -> Result<(), String> {
    if run_ok("nmcli", &["radio", "wifi", if enabled { "on" } else { "off" }]) { Ok(()) } else { Err("nmcli failed".into()) }
}

/// Splits nmcli terse output, which escapes ':' as '\:'.
fn nm_fields(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut esc = false;
    for c in line.chars() {
        if esc {
            cur.push(c);
            esc = false;
        } else if c == '\\' {
            esc = true;
        } else if c == ':' {
            out.push(std::mem::take(&mut cur));
        } else {
            cur.push(c);
        }
    }
    out.push(cur);
    out
}

#[derive(Serialize, Clone)]
pub struct WifiNetwork {
    ssid: String,
    signal: u32,
    secured: bool,
    connected: bool,
    known: bool,
}

pub fn wifi_scan() -> Vec<WifiNetwork> {
    let known: Vec<String> = run("nmcli", &["-t", "-f", "NAME,TYPE", "connection", "show"])
        .unwrap_or_default()
        .lines()
        .map(nm_fields)
        .filter(|f| f.get(1).is_some_and(|t| t.contains("wireless")))
        .map(|f| f[0].clone())
        .collect();
    let out = run("nmcli", &["-t", "-f", "IN-USE,SSID,SIGNAL,SECURITY", "device", "wifi", "list", "--rescan", "auto"])
        .unwrap_or_default();
    let mut list: Vec<WifiNetwork> = Vec::new();
    for line in out.lines() {
        let f = nm_fields(line);
        if f.len() < 4 || f[1].is_empty() {
            continue;
        }
        let net = WifiNetwork {
            connected: f[0].trim() == "*",
            ssid: f[1].clone(),
            signal: f[2].parse().unwrap_or(0),
            secured: !f[3].trim().is_empty() && f[3].trim() != "--",
            known: known.contains(&f[1]),
        };
        match list.iter_mut().find(|n| n.ssid == net.ssid) {
            Some(existing) => {
                if net.signal > existing.signal {
                    existing.signal = net.signal;
                }
                existing.connected |= net.connected;
            }
            None => list.push(net),
        }
    }
    list.sort_by(|a, b| b.connected.cmp(&a.connected).then(b.signal.cmp(&a.signal)));
    list
}

pub fn wifi_connect(ssid: &str, password: Option<&str>) -> Result<(), String> {
    let mut args = vec!["device", "wifi", "connect", ssid];
    if let Some(p) = password.filter(|p| !p.is_empty()) {
        args.push("password");
        args.push(p);
    }
    let out = std::process::Command::new("nmcli").args(&args).output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

fn wifi_device() -> Option<String> {
    run("nmcli", &["-t", "-f", "DEVICE,TYPE,STATE", "device"])?
        .lines()
        .map(nm_fields)
        .find(|f| f.get(1).is_some_and(|t| t == "wifi") && f.get(2).is_some_and(|s| s == "connected"))
        .map(|f| f[0].clone())
}

pub fn wifi_disconnect() -> Result<(), String> {
    let dev = wifi_device().ok_or("Not connected")?;
    if run_ok("nmcli", &["device", "disconnect", &dev]) { Ok(()) } else { Err("nmcli failed".into()) }
}

pub fn wifi_forget(ssid: &str) -> Result<(), String> {
    if run_ok("nmcli", &["connection", "delete", "id", ssid]) { Ok(()) } else { Err("nmcli failed".into()) }
}

#[derive(Serialize)]
pub struct LanStatus {
    present: bool,
    connected: bool,
    name: String,
}

pub fn lan_status() -> LanStatus {
    let mut status = LanStatus { present: false, connected: false, name: String::new() };
    for f in run("nmcli", &["-t", "-f", "DEVICE,TYPE,STATE,CONNECTION", "device"]).unwrap_or_default().lines().map(nm_fields) {
        if f.get(1).is_some_and(|t| t == "ethernet") {
            status.present = true;
            if f.get(2).is_some_and(|s| s == "connected") {
                status.connected = true;
                status.name = f.get(3).cloned().unwrap_or_default();
            }
        }
    }
    status
}

// ---------------------------------------------------------------- bluetooth

pub fn bluetooth_powered() -> bool {
    run("bluetoothctl", &["show"]).is_some_and(|s| s.lines().any(|l| l.trim() == "Powered: yes"))
}

pub fn set_bluetooth(enabled: bool) -> Result<(), String> {
    if enabled {
        run_ok("rfkill", &["unblock", "bluetooth"]);
    }
    if run_ok("bluetoothctl", &["power", if enabled { "on" } else { "off" }]) { Ok(()) } else { Err("bluetoothctl failed".into()) }
}

#[derive(Serialize)]
pub struct BluetoothDeviceInfo {
    id: String,
    name: String,
    connected: bool,
}

pub fn bluetooth_devices() -> Vec<BluetoothDeviceInfo> {
    let list = run("bluetoothctl", &["devices", "Paired"])
        .filter(|s| !s.is_empty())
        .or_else(|| run("bluetoothctl", &["paired-devices"]))
        .unwrap_or_default();
    list.lines()
        .filter_map(|l| {
            let mut parts = l.splitn(3, ' ');
            (parts.next()? == "Device").then_some(())?;
            let id = parts.next()?.to_string();
            let name = parts.next().unwrap_or(&id).to_string();
            let connected = run("bluetoothctl", &["info", &id]).is_some_and(|i| i.contains("Connected: yes"));
            Some(BluetoothDeviceInfo { id, name, connected })
        })
        .collect()
}

pub fn bluetooth_unpair(id: &str) -> Result<(), String> {
    if run_ok("bluetoothctl", &["remove", id]) { Ok(()) } else { Err("bluetoothctl failed".into()) }
}

// ---------------------------------------------------------------- metrics

static CPU_PREV: Mutex<Option<(u64, u64)>> = Mutex::new(None);
static NET_PREV: Mutex<Option<(std::time::Instant, u64, u64)>> = Mutex::new(None);

pub fn cpu_usage() -> u32 {
    let stat = std::fs::read_to_string("/proc/stat").unwrap_or_default();
    let Some(line) = stat.lines().next() else { return 0 };
    let nums: Vec<u64> = line.split_whitespace().skip(1).filter_map(|n| n.parse().ok()).collect();
    if nums.len() < 4 {
        return 0;
    }
    let idle = nums[3] + nums.get(4).copied().unwrap_or(0);
    let total: u64 = nums.iter().take(8).sum();
    let mut prev = CPU_PREV.lock().unwrap_or_else(|e| e.into_inner());
    let usage = match *prev {
        Some((pt, pi)) if total > pt => {
            let dt = total - pt;
            let di = idle.saturating_sub(pi);
            (((dt - di.min(dt)) as f64 / dt as f64) * 100.0).round() as u32
        }
        _ => 0,
    };
    *prev = Some((total, idle));
    usage
}

fn meminfo() -> (u64, u64) {
    let info = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
    let get = |k: &str| {
        info.lines()
            .find(|l| l.starts_with(k))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0)
    };
    (get("MemTotal:"), get("MemAvailable:"))
}

/// RAM in use, percent.
pub fn ram_usage() -> f32 {
    let (total, avail) = meminfo();
    if total == 0 {
        return 0.0;
    }
    ((total - avail.min(total)) as f64 / total as f64 * 100.0) as f32
}

/// Free space on the home filesystem in GB.
pub fn disk_free_gb() -> u64 {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/".into());
    let Ok(c) = std::ffi::CString::new(home) else { return 0 };
    let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c.as_ptr(), &mut st) } != 0 {
        return 0;
    }
    (st.f_bavail as u64 * st.f_frsize as u64) / 1_000_000_000
}

/// (upload, download) bytes per second since the last call.
pub fn network_speed() -> (u64, u64) {
    let dev = std::fs::read_to_string("/proc/net/dev").unwrap_or_default();
    let (mut rx, mut tx) = (0u64, 0u64);
    for line in dev.lines().skip(2) {
        let Some((name, rest)) = line.split_once(':') else { continue };
        if name.trim() == "lo" {
            continue;
        }
        let nums: Vec<u64> = rest.split_whitespace().filter_map(|n| n.parse().ok()).collect();
        if nums.len() >= 9 {
            rx += nums[0];
            tx += nums[8];
        }
    }
    let now = std::time::Instant::now();
    let mut prev = NET_PREV.lock().unwrap_or_else(|e| e.into_inner());
    let speed = match *prev {
        Some((at, prx, ptx)) => {
            let secs = now.duration_since(at).as_secs_f64().max(0.001);
            ((tx.saturating_sub(ptx) as f64 / secs) as u64, (rx.saturating_sub(prx) as f64 / secs) as u64)
        }
        None => (0, 0),
    };
    *prev = Some((now, rx, tx));
    speed
}

pub fn ping_ms(host: &str) -> Option<u32> {
    let out = run("ping", &["-c", "1", "-W", "2", host])?;
    let pos = out.find("time=")?;
    let rest = &out[pos + 5..];
    let num: String = rest.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
    num.parse::<f64>().ok().map(|v| v.round() as u32)
}

#[derive(Serialize, Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub exe_path: Option<String>,
    pub memory_kb: u64,
}

pub fn processes() -> Vec<ProcessInfo> {
    let uid = unsafe { libc::getuid() };
    let mut list = Vec::new();
    let Ok(dir) = std::fs::read_dir("/proc") else { return list };
    for e in dir.flatten() {
        let Ok(pid) = e.file_name().to_string_lossy().parse::<u32>() else { continue };
        let status = std::fs::read_to_string(e.path().join("status")).unwrap_or_default();
        let field = |k: &str| status.lines().find(|l| l.starts_with(k)).map(|l| l[k.len()..].trim().to_string());
        // Only the user's own processes, like Task Manager's default view.
        let owner = field("Uid:").and_then(|u| u.split_whitespace().next().and_then(|v| v.parse::<u32>().ok()));
        if owner != Some(uid) {
            continue;
        }
        let memory_kb = field("VmRSS:").and_then(|v| v.split_whitespace().next().and_then(|n| n.parse().ok())).unwrap_or(0);
        if memory_kb == 0 {
            continue;
        }
        let name = field("Name:").unwrap_or_default();
        let exe_path = std::fs::read_link(e.path().join("exe")).ok().map(|p| p.to_string_lossy().to_string());
        list.push(ProcessInfo { pid, name, exe_path, memory_kb });
    }
    list.sort_by(|a, b| b.memory_kb.cmp(&a.memory_kb));
    list
}

pub fn kill_process(pid: u32) -> Result<(), String> {
    if pid == std::process::id() {
        return Err("Refusing to end Nectar itself".into());
    }
    if unsafe { libc::kill(pid as i32, libc::SIGTERM) } == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error().to_string())
    }
}

pub fn process_exe(pid: u32) -> Option<String> {
    std::fs::read_link(format!("/proc/{pid}/exe")).ok().map(|p| p.to_string_lossy().to_string())
}

// ---------------------------------------------------------------- drives

#[derive(Serialize)]
pub struct DockDrive {
    letter: String,
    label: String,
    removable: bool,
}

/// Mounted removable and user volumes (under /media, /run/media, /mnt).
pub fn drives() -> Vec<DockDrive> {
    let mounts = std::fs::read_to_string("/proc/mounts").unwrap_or_default();
    let mut out = vec![DockDrive { letter: std::env::var("HOME").unwrap_or_else(|_| "/".into()), label: "Home".into(), removable: false }];
    for line in mounts.lines() {
        let mut f = line.split_whitespace();
        let (Some(dev), Some(mnt)) = (f.next(), f.next()) else { continue };
        let mnt = mnt.replace("\\040", " ");
        if !(mnt.starts_with("/media/") || mnt.starts_with("/run/media/") || mnt.starts_with("/mnt/")) || !dev.starts_with("/dev/") {
            continue;
        }
        let devname = dev.trim_start_matches("/dev/");
        let base: String = devname.trim_end_matches(|c: char| c.is_ascii_digit()).trim_end_matches('p').to_string();
        let removable = std::fs::read_to_string(format!("/sys/block/{base}/removable")).is_ok_and(|v| v.trim() == "1");
        let label = std::path::Path::new(&mnt).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| mnt.clone());
        out.push(DockDrive { letter: mnt, label, removable });
    }
    out
}

// ---------------------------------------------------------------- accent

/// Accent color from the GTK/Cinnamon theme name, best effort.
pub fn accent_color() -> Option<String> {
    if let Some(v) = run("gsettings", &["get", "org.gnome.desktop.interface", "accent-color"]) {
        let named = match v.trim_matches('\'') {
            "blue" => Some("#3584e4"), "teal" => Some("#2190a4"), "green" => Some("#3a944a"),
            "yellow" => Some("#c88800"), "orange" => Some("#ed5b00"), "red" => Some("#e62d42"),
            "pink" => Some("#d56199"), "purple" => Some("#9141ac"), "slate" => Some("#6f8396"),
            _ => None,
        };
        if let Some(n) = named {
            return Some(n.into());
        }
    }
    let theme = run("gsettings", &["get", "org.cinnamon.desktop.interface", "gtk-theme"])?.trim_matches('\'').to_lowercase();
    let color = [
        ("aqua", "#1f9ede"), ("blue", "#3584e4"), ("brown", "#b5834a"), ("grey", "#8c8c8c"),
        ("orange", "#ff7139"), ("pink", "#e54980"), ("purple", "#8c5dd9"), ("red", "#e82127"),
        ("sand", "#c5a07c"), ("teal", "#199ca8"),
    ]
    .iter()
    .find(|(k, _)| theme.contains(k))
    .map(|(_, c)| *c)
    .unwrap_or(if theme.starts_with("mint") { "#35a854" } else { "#3584e4" });
    Some(color.into())
}
