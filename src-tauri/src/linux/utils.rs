//! Settings cache and small helpers shared by the Linux backend.
use std::collections::HashMap;
use tauri::Manager;

pub fn get_now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn settings_cache() -> &'static std::sync::Mutex<HashMap<String, serde_json::Value>> {
    crate::state::SETTINGS_CACHE.get_or_init(|| std::sync::Mutex::new(HashMap::new()))
}

pub fn settings_path(app: &tauri::AppHandle) -> Option<std::path::PathBuf> {
    app.path().app_config_dir().ok().map(|p| p.join("settings.json"))
}

pub fn read_settings_file(app: &tauri::AppHandle) -> HashMap<String, serde_json::Value> {
    settings_path(app)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or_default()
}

/// Load settings.json into the in-memory cache. Call once at startup.
pub fn init_settings_cache(app: &tauri::AppHandle) {
    replace_settings_cache(read_settings_file(app));
}

pub fn replace_settings_cache(new_settings: HashMap<String, serde_json::Value>) {
    if let Ok(mut cache) = settings_cache().lock() {
        *cache = new_settings;
    }
}

pub fn get_setting_str(_app: &tauri::AppHandle, key: &str) -> Option<String> {
    setting_str(key)
}

pub fn setting_str(key: &str) -> Option<String> {
    let guard = settings_cache().lock().ok()?;
    match guard.get(key)? {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Bool(b) => Some(b.to_string()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

pub fn get_nectar_scale(app: &tauri::AppHandle) -> f64 {
    get_setting_str(app, "nectar-scale")
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(1.0)
}

pub fn png_data_url(bytes: &[u8]) -> String {
    use base64::Engine;
    format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes))
}

pub fn rgba_to_png_data_url(img: image::RgbaImage) -> Option<String> {
    let mut out = Vec::new();
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .ok()?;
    Some(png_data_url(&out))
}

/// Reads any raster or SVG image and returns a PNG data URL no larger than `max` px.
pub fn image_file_to_data_url(path: &std::path::Path, max: u32) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    if bytes.len() > 8 * 1024 * 1024 {
        return None;
    }
    let is_svg = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("svg") || e.eq_ignore_ascii_case("svgz"));
    if is_svg {
        return svg_to_png_data_url(&bytes, max);
    }
    let img = image::load_from_memory(&bytes).ok()?;
    let img = if img.width() > max || img.height() > max { img.thumbnail(max, max) } else { img };
    rgba_to_png_data_url(img.to_rgba8())
}

pub fn svg_to_png_data_url(bytes: &[u8], size: u32) -> Option<String> {
    let opt = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_data(bytes, &opt).ok()?;
    let ts = tree.size();
    let scale = size as f32 / ts.width().max(ts.height()).max(1.0);
    let w = ((ts.width() * scale).ceil() as u32).max(1);
    let h = ((ts.height() * scale).ceil() as u32).max(1);
    let mut pixmap = resvg::tiny_skia::Pixmap::new(w, h)?;
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    pixmap.encode_png().ok().map(|png| png_data_url(&png))
}

/// Runs a command and returns trimmed stdout when it exits successfully.
pub fn run(cmd: &str, args: &[&str]) -> Option<String> {
    let out = std::process::Command::new(cmd).args(args).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Runs a command and reports only whether it succeeded.
pub fn run_ok(cmd: &str, args: &[&str]) -> bool {
    std::process::Command::new(cmd)
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Starts a program detached from Nectar.
pub fn spawn_detached(cmd: &str, args: &[&str]) -> bool {
    use std::os::unix::process::CommandExt;
    std::process::Command::new(cmd)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .process_group(0)
        .spawn()
        .is_ok()
}

pub fn which(cmd: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|p| p.join(cmd).is_file()))
        .unwrap_or(false)
}
