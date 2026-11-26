//! Now-playing info and controls through MPRIS (via playerctl).

use crate::types::MediaInfo;
use crate::utils::{run, run_ok};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};

pub static SELECTED: Mutex<Option<String>> = Mutex::new(None);
pub static ACTIVE: Mutex<String> = Mutex::new(String::new());
pub static LAST: Mutex<Option<MediaInfo>> = Mutex::new(None);
static ART_CACHE: Mutex<Option<(String, String)>> = Mutex::new(None);

#[derive(Clone, Debug)]
pub struct Player {
    pub id: String,
    pub status: String,
    pub title: String,
    pub artist: String,
    pub art_url: String,
    pub length_us: i64,
    pub position_us: i64,
}

const FIELDS: &str = "{{playerInstance}}\u{1f}{{status}}\u{1f}{{xesam:title}}\u{1f}{{xesam:artist}}\u{1f}{{mpris:artUrl}}\u{1f}{{mpris:length}}\u{1f}{{position}}";

pub fn players() -> Vec<Player> {
    let Some(out) = run("playerctl", &["-a", "metadata", "--format", FIELDS]) else { return Vec::new() };
    out.lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.split('\u{1f}').collect();
            if f.len() < 7 {
                return None;
            }
            Some(Player {
                id: f[0].to_string(),
                status: f[1].to_string(),
                title: f[2].to_string(),
                artist: f[3].to_string(),
                art_url: f[4].to_string(),
                length_us: f[5].parse().unwrap_or(0),
                position_us: f[6].parse().unwrap_or(0),
            })
        })
        .filter(|p| !p.title.is_empty())
        .collect()
}

/// Friendly name from an MPRIS instance like "spotify" or "firefox.instance_1_23".
pub fn app_name(id: &str) -> String {
    let base = id.split('.').next().unwrap_or(id);
    let mut c = base.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => id.to_string(),
    }
}

fn artwork(url: &str) -> Option<String> {
    if url.is_empty() {
        return None;
    }
    if let Ok(guard) = ART_CACHE.lock() {
        if let Some((u, data)) = guard.as_ref() {
            if u == url {
                return Some(data.clone());
            }
        }
    }
    let bytes = if let Some(path) = url.strip_prefix("file://") {
        let decoded = percent_decode(path);
        std::fs::read(decoded).ok()?
    } else if url.starts_with("http://") || url.starts_with("https://") {
        let resp = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .ok()?
            .get(url)
            .send()
            .ok()?;
        if !resp.status().is_success() {
            return None;
        }
        resp.bytes().ok()?.to_vec()
    } else if let Some(b64) = url.strip_prefix("data:") {
        return Some(format!("data:{b64}"));
    } else {
        return None;
    };
    let img = image::load_from_memory(&bytes).ok()?;
    let img = if img.width() > 512 { img.thumbnail(512, 512) } else { img };
    let data = crate::utils::rgba_to_png_data_url(img.to_rgba8())?;
    if let Ok(mut guard) = ART_CACHE.lock() {
        *guard = Some((url.to_string(), data.clone()));
    }
    Some(data)
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

fn chosen(players: &[Player]) -> Option<Player> {
    let selected = SELECTED.lock().ok().and_then(|g| g.clone());
    selected
        .and_then(|id| players.iter().find(|p| p.id == id).cloned())
        .or_else(|| players.iter().find(|p| p.status == "Playing").cloned())
        .or_else(|| players.first().cloned())
}

fn target_args(cmd: &str) -> Vec<String> {
    let active = ACTIVE.lock().map(|g| g.clone()).unwrap_or_default();
    let mut args = Vec::new();
    if !active.is_empty() {
        args.push("-p".to_string());
        args.push(active);
    }
    args.push(cmd.to_string());
    args
}

pub fn control(cmd: &str) {
    let args = target_args(cmd);
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    run_ok("playerctl", &refs);
}

pub fn seek(position_ms: i64) {
    let mut args = target_args("position");
    args.push(format!("{:.3}", position_ms.max(0) as f64 / 1000.0));
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    run_ok("playerctl", &refs);
}

pub fn control_player(id: &str, action: &str) -> Result<(), String> {
    let cmd = match action {
        "play_pause" => "play-pause",
        "next" => "next",
        "previous" => "previous",
        _ => return Err("Unknown media action".into()),
    };
    if run_ok("playerctl", &["-p", id, cmd]) { Ok(()) } else { Err("No media session for that app".into()) }
}

/// Polls players once a second and emits "media-update" / "media-sessions" on change.
pub fn start_watcher(app: AppHandle) {
    if !crate::utils::which("playerctl") {
        crate::diagnostics::log("media: playerctl not found, media controls are off");
        return;
    }
    std::thread::spawn(move || {
        let mut last_sessions = String::new();
        let mut last_key: Option<(String, String, bool, bool, String, i64)> = None;
        loop {
            if crate::state::SHUTTING_DOWN.load(std::sync::atomic::Ordering::Relaxed) {
                return;
            }
            let list = players();
            let current = chosen(&list);
            let active_id = current.as_ref().map(|p| p.id.clone()).unwrap_or_default();
            if let Ok(mut g) = ACTIVE.lock() {
                *g = active_id.clone();
            }
            let sessions: Vec<serde_json::Value> = list
                .iter()
                .map(|p| serde_json::json!({ "id": p.id, "name": app_name(&p.id), "title": p.title, "is_playing": p.status == "Playing", "active": p.id == active_id }))
                .collect();
            let sessions_json = serde_json::to_string(&sessions).unwrap_or_default();
            if sessions_json != last_sessions {
                let _ = app.emit("media-sessions", sessions);
                last_sessions = sessions_json;
            }

            let info = match &current {
                Some(p) => MediaInfo {
                    title: p.title.clone(),
                    artist: p.artist.clone(),
                    is_playing: p.status == "Playing",
                    has_media: true,
                    artwork: artwork(&p.art_url).map(|a| vec![a]),
                    position_ms: p.position_us / 1000,
                    duration_ms: p.length_us / 1000,
                    seek_enabled: p.length_us > 0,
                    position_updated_at: crate::utils::get_now_ms() as u64,
                    source_id: p.id.clone(),
                },
                None => MediaInfo {
                    title: String::new(), artist: String::new(), is_playing: false, has_media: false, artwork: None,
                    position_ms: 0, duration_ms: 0, seek_enabled: false, position_updated_at: 0, source_id: String::new(),
                },
            };
            let art = info.artwork.as_ref().and_then(|a| a.first()).cloned().unwrap_or_default();
            let changed = last_key.as_ref().is_none_or(|(t, a, p, h, art_prev, pos)| {
                t != &info.title || a != &info.artist || *p != info.is_playing || *h != info.has_media || art_prev != &art
                    || (pos - info.position_ms).abs() > 1500
            });
            if changed {
                let art_is_new = last_key.as_ref().is_none_or(|k| k.4 != art);
                let mut payload = info.clone();
                if !art_is_new {
                    payload.artwork = None;
                }
                if let Ok(mut g) = LAST.lock() {
                    *g = Some(info.clone());
                }
                let _ = app.emit("media-update", payload);
                last_key = Some((info.title, info.artist, info.is_playing, info.has_media, art, info.position_ms));
            } else if let Some(k) = last_key.as_mut() {
                // Track position so a steady playback doesn't look like a seek.
                k.5 = info.position_ms;
            }
            std::thread::sleep(std::time::Duration::from_millis(1000));
        }
    });
}
