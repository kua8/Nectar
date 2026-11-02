// Desktop wallpaper for the glass styles. The notch and dock windows are transparent, so they
// can't blur what's behind them; instead they draw a pre-blurred copy of the wallpaper, lined up
// with where they sit on the screen.

use serde::Serialize;
use std::sync::Mutex;
use std::time::SystemTime;
use tauri::Window;

// The copy is blurred and only ever seen behind the notch and dock, so it can be small.
const BACKDROP_SIZE: u32 = 640;
const CACHE_LIMIT: usize = 4;

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WallpaperBackdrop {
    /// Downscaled, blurred JPEG as a data URL
    image: String,
    /// Original image size, to work out how Windows fits it to the screen
    image_width: u32,
    image_height: u32,
    /// "fill", "fit", "stretch", "center", "tile" or "span"
    fit: String,
    /// The monitor, and this window's offset from its top-left corner, in logical pixels
    monitor_width: f64,
    monitor_height: f64,
    monitor_scale: f64,
    window_x: f64,
    window_y: f64,
    window_width: f64,
    window_height: f64,
}

struct CachedImage {
    path: String,
    modified: Option<SystemTime>,
    blur: u32,
    image: String,
    width: u32,
    height: u32,
}

static CACHE: Mutex<Vec<CachedImage>> = Mutex::new(Vec::new());

fn wallpaper_path() -> Option<String> {
    use windows::Win32::UI::WindowsAndMessaging::{
        SystemParametersInfoW, SPI_GETDESKWALLPAPER, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
    };
    let mut buf = [0u16; 1024];
    unsafe {
        SystemParametersInfoW(
            SPI_GETDESKWALLPAPER,
            buf.len() as u32,
            Some(buf.as_mut_ptr() as *mut core::ffi::c_void),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
        .ok()?;
    }
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    let path = String::from_utf16_lossy(&buf[..len]);
    if path.trim().is_empty() {
        None
    } else {
        Some(path)
    }
}

fn read_desktop_string(name: windows::core::PCWSTR) -> Option<String> {
    use windows::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ, REG_VALUE_TYPE,
    };
    unsafe {
        let mut hkey = HKEY::default();
        if RegOpenKeyExW(HKEY_CURRENT_USER, windows::core::w!("Control Panel\\Desktop"), Some(0), KEY_READ, &mut hkey).is_err() {
            return None;
        }
        let mut buf = [0u16; 64];
        let mut size = std::mem::size_of_val(&buf) as u32;
        let mut kind = REG_VALUE_TYPE::default();
        let result = RegQueryValueExW(hkey, name, None, Some(&mut kind), Some(buf.as_mut_ptr() as *mut u8), Some(&mut size));
        let _ = RegCloseKey(hkey);
        if result.is_err() {
            return None;
        }
        let len = (size as usize / 2).min(buf.len());
        Some(String::from_utf16_lossy(&buf[..len]).trim_end_matches('\0').trim().to_string())
    }
}

// How Windows draws the wallpaper (Settings › Personalization › Background › "Choose a fit")
fn wallpaper_fit() -> String {
    let style = read_desktop_string(windows::core::w!("WallpaperStyle")).unwrap_or_default();
    let tile = read_desktop_string(windows::core::w!("TileWallpaper")).unwrap_or_default();
    match (style.as_str(), tile.as_str()) {
        ("0", "1") => "tile",
        ("0", _) => "center",
        ("2", _) => "stretch",
        ("6", _) => "fit",
        ("22", _) => "span",
        _ => "fill",
    }
    .to_string()
}

fn load_blurred(path: &str, blur: u32) -> Result<(String, u32, u32), String> {
    let modified = std::fs::metadata(path).and_then(|m| m.modified()).ok();
    if let Ok(cache) = CACHE.lock() {
        if let Some(hit) = cache.iter().find(|c| c.path == path && c.modified == modified && c.blur == blur) {
            return Ok((hit.image.clone(), hit.width, hit.height));
        }
    }

    let img = image::open(path).map_err(|e| e.to_string())?;
    let (width, height) = (img.width(), img.height());
    let small = img.thumbnail(BACKDROP_SIZE, BACKDROP_SIZE);
    let small = if blur > 0 { small.blur(blur as f32) } else { small };

    let mut out = Vec::new();
    image::DynamicImage::ImageRgb8(small.to_rgb8())
        .write_with_encoder(image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 85))
        .map_err(|e| e.to_string())?;
    use base64::Engine;
    let data = format!("data:image/jpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode(&out));

    if let Ok(mut cache) = CACHE.lock() {
        cache.retain(|c| !(c.path == path && c.blur == blur));
        if cache.len() >= CACHE_LIMIT {
            cache.remove(0);
        }
        cache.push(CachedImage { path: path.to_string(), modified, blur, image: data.clone(), width, height });
    }
    Ok((data, width, height))
}

/// The wallpaper behind the calling window, blurred by `blur` pixels of the small copy.
/// Returns None when there is no image wallpaper (a solid color, or a file that can't be read).
#[tauri::command]
pub async fn get_wallpaper_backdrop(window: Window, blur: u32) -> Result<Option<WallpaperBackdrop>, String> {
    let Some(path) = wallpaper_path() else { return Ok(None) };
    let Some(monitor) = window.current_monitor().map_err(|e| e.to_string())? else { return Ok(None) };
    let scale = monitor.scale_factor();
    let m_pos = *monitor.position();
    let m_size = *monitor.size();
    let w_pos = window.outer_position().map_err(|e| e.to_string())?;
    let w_size = window.outer_size().map_err(|e| e.to_string())?;
    let fit = wallpaper_fit();

    let blur = blur.min(40);
    let loaded = tauri::async_runtime::spawn_blocking(move || load_blurred(&path, blur))
        .await
        .map_err(|e| e.to_string())?;
    let Ok((image, image_width, image_height)) = loaded else { return Ok(None) };

    Ok(Some(WallpaperBackdrop {
        image,
        image_width,
        image_height,
        fit,
        monitor_width: m_size.width as f64 / scale,
        monitor_height: m_size.height as f64 / scale,
        monitor_scale: scale,
        window_x: (w_pos.x - m_pos.x) as f64 / scale,
        window_y: (w_pos.y - m_pos.y) as f64 / scale,
        window_width: w_size.width as f64 / scale,
        window_height: w_size.height as f64 / scale,
    }))
}
