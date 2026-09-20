use windows::Win32::Foundation::{HWND, HGLOBAL};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL, IPersistFile, CLSCTX_INPROC_SERVER};
use windows::Win32::System::Com::StructuredStorage::{CreateStreamOnHGlobal, GetHGlobalFromStream};
use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};
use windows::core::Interface;
use windows::Win32::UI::WindowsAndMessaging::HICON;
use windows::Win32::Graphics::Imaging::{IWICImagingFactory, CLSID_WICImagingFactory, GUID_ContainerFormatPng, WICBitmapEncoderNoCache, GUID_WICPixelFormat32bppPBGRA};
use base64::{Engine as _, engine::general_purpose};
use std::path::PathBuf;
use std::sync::OnceLock;
use tauri::Manager;

pub fn resolve_shortcut(path: &str) -> Option<(String, String)> {
    unsafe {
        let shell_link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_ALL).ok()?;
        let persist_file: IPersistFile = shell_link.cast().ok()?;
        
        let wide_path: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
        persist_file.Load(windows::core::PCWSTR(wide_path.as_ptr()), windows::Win32::System::Com::STGM(0)).ok()?;
        
        let _ = shell_link.Resolve(HWND(std::ptr::null_mut()), 1 | 16 | 32); 
        
        let mut buffer = [0u16; 260];
        let mut data = windows::Win32::Storage::FileSystem::WIN32_FIND_DATAW::default();
        shell_link.GetPath(&mut buffer, &mut data, 0).ok()?;
        
        let mut arg_buffer = [0u16; 1024];
        let _ = shell_link.GetArguments(&mut arg_buffer);

        let target = String::from_utf16_lossy(&buffer).trim_matches(char::from(0)).to_string();
        let args = String::from_utf16_lossy(&arg_buffer).trim_matches(char::from(0)).to_string();
        
        if target.trim().is_empty() { None } else { Some((target, args)) }
    }
}

pub static ORIGINAL_TRAY_RECT: std::sync::Mutex<Option<windows::Win32::Foundation::RECT>> = std::sync::Mutex::new(None);
static ORIGINAL_SEC_TRAY_RECT: std::sync::Mutex<Option<windows::Win32::Foundation::RECT>> = std::sync::Mutex::new(None);
static ORIGINAL_TASKBAR_STATE: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(-1);

static TASKBAR_MARKER: OnceLock<PathBuf> = OnceLock::new();

/// Point the crash-recovery marker at this user's app config dir (called from setup).
pub fn init_taskbar_marker(app: &tauri::AppHandle) {
    if let Ok(dir) = app.path().app_config_dir() {
        let _ = TASKBAR_MARKER.set(dir.join("taskbar_hidden.flag"));
    }
}

/// True if a previous Nectar session died without restoring the native taskbar.
pub fn taskbar_marker_exists() -> bool {
    TASKBAR_MARKER.get().is_some_and(|p| p.exists())
}

pub fn set_taskbar_visibility(visible: bool, always_on_top: bool) {
    // Crash-recovery marker: a hidden taskbar is persisted so the next launch can
    // undo it if we're ever force-killed (Task Manager / TerminateProcess skips cleanup).
    if visible {
        if let Some(p) = TASKBAR_MARKER.get() { let _ = std::fs::remove_file(p); }
    } else {
        if let Some(p) = TASKBAR_MARKER.get() { if !p.exists() { let _ = std::fs::write(p, b"1"); } }
    }

    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::{FindWindowA, ShowWindow, SW_HIDE, SW_SHOW, GetWindowRect};
        use windows::Win32::UI::Shell::{SHAppBarMessage, APPBARDATA, ABM_SETSTATE, ABM_GETSTATE};

        let tray_class = windows::core::PCSTR(c"Shell_TrayWnd".as_ptr() as *const u8);
        let secondary_tray_class = windows::core::PCSTR(c"Shell_SecondaryTrayWnd".as_ptr() as *const u8);

        // Save original taskbar state before modifying
        if ORIGINAL_TASKBAR_STATE.load(std::sync::atomic::Ordering::Relaxed) == -1 {
            let mut get_abd = APPBARDATA { cbSize: std::mem::size_of::<APPBARDATA>() as u32, ..Default::default() };
            let original_state = SHAppBarMessage(ABM_GETSTATE, &mut get_abd);
            ORIGINAL_TASKBAR_STATE.store(original_state as i32, std::sync::atomic::Ordering::Relaxed);
        }

        let state_val = if visible {
            let orig = ORIGINAL_TASKBAR_STATE.load(std::sync::atomic::Ordering::Relaxed);
            if orig != -1 { orig as isize } else { if always_on_top { 2 } else { 1 } }
        } else {
            1 // Force Auto-hide when hiding
        };

        // 1. Set the taskbar state (Auto-hide or Always-on-top)
        let mut abd = APPBARDATA { 
            cbSize: std::mem::size_of::<APPBARDATA>() as u32, 
            lParam: windows::Win32::Foundation::LPARAM(state_val), 
            ..Default::default() 
        };
        SHAppBarMessage(ABM_SETSTATE, &mut abd);

        // 2. Control visibility of the primary taskbar
        if let Ok(tray_hwnd) = FindWindowA(tray_class, windows::core::PCSTR::null()) {
            use windows::Win32::UI::WindowsAndMessaging::{SetWindowPos, SWP_NOSIZE, SWP_NOZORDER, SWP_NOACTIVATE, GetWindowLongA, SetWindowLongA, GWL_EXSTYLE, WS_EX_LAYERED, WS_EX_TRANSPARENT, SetLayeredWindowAttributes, LWA_ALPHA};
            if visible {
                // Revert any lingering WS_EX_LAYERED / WS_EX_TRANSPARENT left by
                // open_system_tray if the user quit before the tray thread cleaned up.
                let ex = GetWindowLongA(tray_hwnd, GWL_EXSTYLE);
                let cleaned = ex & !(WS_EX_LAYERED.0 as i32) & !(WS_EX_TRANSPARENT.0 as i32);
                if cleaned != ex {
                    let _ = SetWindowLongA(tray_hwnd, GWL_EXSTYLE, cleaned);
                    let _ = SetLayeredWindowAttributes(tray_hwnd, windows::Win32::Foundation::COLORREF(0), 255, LWA_ALPHA);
                }
                if let Ok(guard) = ORIGINAL_TRAY_RECT.lock() {
                    if let Some(rect) = *guard {
                        let _ = SetWindowPos(tray_hwnd, None, rect.left, rect.top, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
                    }
                }
                let _ = ShowWindow(tray_hwnd, SW_SHOW);

                // ShowWindow/SetWindowPos can put the taskbar back in the right place
                // without Explorer actually repainting it, since this is raw window
                // manipulation entirely outside Explorer's own visibility bookkeeping
                // (that's driven by the AppBar API, which auto-hide state changes
                // normally go through). Force a repaint and nudge Explorer's
                // tray-settings listener directly instead of relying on it noticing
                // on its own, so a restart of Explorer is never required.
                {
                    use windows::Win32::Graphics::Gdi::{RedrawWindow, RDW_INVALIDATE, RDW_ALLCHILDREN, RDW_UPDATENOW, RDW_FRAME, RDW_ERASE};
                    let _ = RedrawWindow(Some(tray_hwnd), None, None, RDW_INVALIDATE | RDW_ALLCHILDREN | RDW_UPDATENOW | RDW_FRAME | RDW_ERASE);

                    use windows::Win32::UI::WindowsAndMessaging::{SendMessageTimeoutA, HWND_BROADCAST, WM_SETTINGCHANGE, SMTO_ABORTIFHUNG};
                    use windows::Win32::Foundation::{WPARAM, LPARAM};
                    let param = c"TraySettings";
                    let mut result: usize = 0;
                    let _ = SendMessageTimeoutA(HWND_BROADCAST, WM_SETTINGCHANGE, WPARAM(0), LPARAM(param.as_ptr() as isize), SMTO_ABORTIFHUNG, 100, Some(&mut result));
                }
            } else {
                let has_rect = ORIGINAL_TRAY_RECT.lock().map(|g| g.is_some()).unwrap_or(false);
                if !has_rect {
                    let mut rect = windows::Win32::Foundation::RECT::default();
                    let _ = GetWindowRect(tray_hwnd, &mut rect);
                    if let Ok(mut guard) = ORIGINAL_TRAY_RECT.lock() {
                        *guard = Some(rect);
                    }
                }
                let _ = ShowWindow(tray_hwnd, SW_HIDE);
                // Move it far off-screen to prevent any "thin line" artifacts or flashes
                let _ = SetWindowPos(tray_hwnd, None, -10000, -10000, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
            }
        }

        // 3. Control visibility of secondary taskbars (multi-monitor)
        if let Ok(secondary_tray_hwnd) = FindWindowA(secondary_tray_class, windows::core::PCSTR::null()) {
            use windows::Win32::UI::WindowsAndMessaging::{SetWindowPos, SWP_NOSIZE, SWP_NOZORDER, SWP_NOACTIVATE};
            if visible {
                if let Ok(guard) = ORIGINAL_SEC_TRAY_RECT.lock() {
                    if let Some(rect) = *guard {
                        let _ = SetWindowPos(secondary_tray_hwnd, None, rect.left, rect.top, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
                    }
                }
                let _ = ShowWindow(secondary_tray_hwnd, SW_SHOW);
                {
                    use windows::Win32::Graphics::Gdi::{RedrawWindow, RDW_INVALIDATE, RDW_ALLCHILDREN, RDW_UPDATENOW, RDW_FRAME, RDW_ERASE};
                    let _ = RedrawWindow(Some(secondary_tray_hwnd), None, None, RDW_INVALIDATE | RDW_ALLCHILDREN | RDW_UPDATENOW | RDW_FRAME | RDW_ERASE);
                }
            } else {
                let has_sec_rect = ORIGINAL_SEC_TRAY_RECT.lock().map(|g| g.is_some()).unwrap_or(false);
                if !has_sec_rect {
                    let mut rect = windows::Win32::Foundation::RECT::default();
                    let _ = GetWindowRect(secondary_tray_hwnd, &mut rect);
                    if let Ok(mut guard) = ORIGINAL_SEC_TRAY_RECT.lock() {
                        *guard = Some(rect);
                    }
                }
                let _ = ShowWindow(secondary_tray_hwnd, SW_HIDE);
                let _ = SetWindowPos(secondary_tray_hwnd, None, -10000, -10000, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
            }
        }

        // Belt-and-suspenders alongside the RedrawWindow calls above: broadcast
        // the same "TraySettings" WM_SETTINGCHANGE other shell-tweaking tools
        // use to get Explorer to resync itself, in case any part of its stale
        // state isn't just a paint issue. SendMessageTimeout (not SendMessage)
        // so a non-responding top-level window can't hang this call.
        if visible {
            use windows::Win32::UI::WindowsAndMessaging::{SendMessageTimeoutW, HWND_BROADCAST, WM_SETTINGCHANGE, SMTO_ABORTIFHUNG};
            let setting: Vec<u16> = "TraySettings".encode_utf16().chain(std::iter::once(0)).collect();
            let _ = SendMessageTimeoutW(
                HWND_BROADCAST,
                WM_SETTINGCHANGE,
                windows::Win32::Foundation::WPARAM(0),
                windows::Win32::Foundation::LPARAM(setting.as_ptr() as isize),
                SMTO_ABORTIFHUNG,
                1000,
                None,
            );
        }
    }
}


pub unsafe fn icon_to_base64(hicon: HICON) -> Option<String> {
    let factory: IWICImagingFactory = CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER).ok()?;
    let bitmap = factory.CreateBitmapFromHICON(hicon).ok()?;
    
    let stream = CreateStreamOnHGlobal(HGLOBAL(std::ptr::null_mut()), true).ok()?;
    let encoder = factory.CreateEncoder(&GUID_ContainerFormatPng, std::ptr::null()).ok()?;
    encoder.Initialize(&stream, WICBitmapEncoderNoCache).ok()?;
    
    let mut frame = None;
    encoder.CreateNewFrame(&mut frame, std::ptr::null_mut()).ok()?;
    let frame = frame?;
    frame.Initialize(None).ok()?;
    
    let (mut width, mut height) = (0u32, 0u32);
    bitmap.GetSize(&mut width, &mut height).ok()?;
    frame.SetSize(width, height).ok()?;
    
    let mut format = GUID_WICPixelFormat32bppPBGRA;
    frame.SetPixelFormat(&mut format).ok()?;
    
    frame.WriteSource(&bitmap, std::ptr::null()).ok()?;
    frame.Commit().ok()?;
    encoder.Commit().ok()?;
    
    let hglobal = GetHGlobalFromStream(&stream).ok()?;
    let ptr = windows::Win32::System::Memory::GlobalLock(hglobal);
    let size = windows::Win32::System::Memory::GlobalSize(hglobal);
    
    let data = std::slice::from_raw_parts(ptr as *const u8, size);
    let png_bytes = data.to_vec();

    let _ = windows::Win32::System::Memory::GlobalUnlock(hglobal);

    let normalized = normalize_icon_padding(&png_bytes).unwrap_or(png_bytes);
    let base64_str = general_purpose::STANDARD.encode(&normalized);

    Some(format!("data:image/png;base64,{}", base64_str))
}

/// Many extracted app icons fill their square canvas almost edge-to-edge,
/// unlike Nectar's own mark which has deliberate breathing
/// room baked in. Rendered at the same dock-tile size, those icons look
/// cramped next to ones with real padding. Trim to the icon's actual visible
/// content and re-center it on a canvas sized so the content never exceeds
/// TARGET_FILL_RATIO of it — icons that already have enough padding are just
/// re-centered (which also fixes off-center source art), never shrunk further.
fn normalize_icon_padding(png_bytes: &[u8]) -> Option<Vec<u8>> {
    const TARGET_FILL_RATIO: f64 = 0.82;

    let img = image::load_from_memory(png_bytes).ok()?;
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();

    let mut minx = w;
    let mut miny = h;
    let mut maxx: i64 = 0;
    let mut maxy: i64 = 0;
    let mut found = false;
    for y in 0..h {
        for x in 0..w {
            if rgba.get_pixel(x, y)[3] > 10 {
                found = true;
                if x < minx { minx = x; }
                if (x as i64) > maxx { maxx = x as i64; }
                if y < miny { miny = y; }
                if (y as i64) > maxy { maxy = y as i64; }
            }
        }
    }
    if !found { return None; }

    let content_w = (maxx as u32).saturating_sub(minx) + 1;
    let content_h = (maxy as u32).saturating_sub(miny) + 1;
    let content_max = content_w.max(content_h);
    let base_size = w.min(h);
    let current_ratio = content_max as f64 / base_size as f64;

    let canvas_size = if current_ratio > TARGET_FILL_RATIO {
        (content_max as f64 / TARGET_FILL_RATIO).round() as u32
    } else {
        base_size
    };

    let cropped = image::imageops::crop_imm(&rgba, minx, miny, content_w, content_h).to_image();
    let mut canvas = image::RgbaImage::new(canvas_size, canvas_size);
    let ox = ((canvas_size - content_w) / 2) as i64;
    let oy = ((canvas_size - content_h) / 2) as i64;
    image::imageops::overlay(&mut canvas, &cropped, ox, oy);

    let mut out: Vec<u8> = Vec::new();
    image::DynamicImage::ImageRgba8(canvas)
        .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .ok()?;
    Some(out)
}

pub fn get_now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as i64
}

/// Load settings.json into the in-memory cache. Call once at startup.
pub fn init_settings_cache(app: &tauri::AppHandle) {
    use tauri::Manager;
    use crate::state::SETTINGS_CACHE;
    let _ = SETTINGS_CACHE.set(std::sync::Mutex::new(std::collections::HashMap::new()));
    if let Some(path) = app.path().app_config_dir().ok().map(|p| p.join("settings.json")) {
        if let Ok(content) = std::fs::read_to_string(path) {
            if let Ok(settings) = serde_json::from_str::<std::collections::HashMap<String, serde_json::Value>>(&content) {
                if let Ok(mut cache) = SETTINGS_CACHE.get().unwrap().lock() {
                    *cache = settings;
                }
            }
        }
    }
}

/// Replace the entire settings cache (used by the file watcher on external changes).
pub fn replace_settings_cache(new_settings: std::collections::HashMap<String, serde_json::Value>) {
    if let Ok(mut cache) = crate::state::SETTINGS_CACHE.get().unwrap().lock() {
        *cache = new_settings;
    }
}

pub fn get_nectar_scale(_app: &tauri::AppHandle) -> f64 {
    get_setting_str(_app, "nectar-scale")
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(1.0)
}

/// Read any string value from the settings cache. Returns None if the key is absent.
pub fn get_setting_str(_app: &tauri::AppHandle, key: &str) -> Option<String> {
    let cache = crate::state::SETTINGS_CACHE.get()?;
    let guard = cache.lock().ok()?;
    guard.get(key)?.as_str().map(|s| s.to_string())
}

/// Re-assert HWND_TOPMOST without activating the window.
///
/// Tauri's `set_always_on_top(true)` calls `SetWindowPos(HWND_TOPMOST)` without
/// `SWP_NOACTIVATE`, which causes Windows to send `WM_ACTIVATE` to the WebView2 window.
/// This activation message makes the WebView compositor briefly blank/hide the window,
/// and can also strip the `WS_EX_NOACTIVATE` extended style.
///
/// This helper uses the raw Win32 call with the correct flags and re-stamps
/// `WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW` to prevent both problems.
pub fn re_assert_topmost(hwnd: HWND) {
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::{
            SetWindowPos, GetWindowLongPtrW, SetWindowLongPtrW,
            HWND_TOPMOST, SWP_NOMOVE, SWP_NOSIZE, SWP_NOACTIVATE, SWP_NOSENDCHANGING,
            GWL_EXSTYLE, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
        };
        // Set topmost without activating or notifying the window
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            0, 0, 0, 0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOSENDCHANGING,
        );
        // Re-stamp NOACTIVATE + TOOLWINDOW — HWND_TOPMOST can cause these to be reset
        // by the shell on some Windows builds
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as usize;
        let new_ex = ex | WS_EX_NOACTIVATE.0 as usize | WS_EX_TOOLWINDOW.0 as usize;
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new_ex as isize);
    }
}

/// Captures an HWND to a base64-encoded PNG thumbnail, scaling it down if it exceeds max_width x max_height.
pub fn capture_hwnd_to_base64(hwnd: HWND, max_width: u32, max_height: u32) -> Option<String> {
    unsafe {
        use windows::Win32::Foundation::RECT;
        use windows::Win32::Graphics::Gdi::{
            CreateCompatibleDC, CreateCompatibleBitmap, SelectObject, DeleteObject, DeleteDC,
            GetDC, ReleaseDC, GetDIBits, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HDC,
        };
        use windows::Win32::UI::WindowsAndMessaging::{GetWindowPlacement, WINDOWPLACEMENT, IsWindow};
        use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS};
        #[link(name = "user32")]
        extern "system" { pub fn PrintWindow(hwnd: HWND, hdcBlt: HDC, nFlags: u32) -> i32; }

        if !IsWindow(Some(hwnd)).as_bool() { return None; }

        let mut rect = RECT::default();
        let _ = DwmGetWindowAttribute(hwnd, DWMWA_EXTENDED_FRAME_BOUNDS, &mut rect as *mut _ as *mut _, std::mem::size_of::<RECT>() as u32);
        if rect.right == 0 && rect.bottom == 0 && windows::Win32::UI::WindowsAndMessaging::GetWindowRect(hwnd, &mut rect).is_err() { return None; }
        let mut width = rect.right - rect.left;
        let mut height = rect.bottom - rect.top;
        if width <= 10 || height <= 10 {
            let mut wp = WINDOWPLACEMENT { length: std::mem::size_of::<WINDOWPLACEMENT>() as u32, ..Default::default() };
            if GetWindowPlacement(hwnd, &mut wp).is_ok() {
                width = wp.rcNormalPosition.right - wp.rcNormalPosition.left;
                height = wp.rcNormalPosition.bottom - wp.rcNormalPosition.top;
            }
        }
        if width <= 100 || height <= 100 || width > 7680 || height > 4320 { return None; }

        let hdc_screen = GetDC(None);
        let hdc_mem = CreateCompatibleDC(Some(hdc_screen));
        let hbm_mem = CreateCompatibleBitmap(hdc_screen, width, height);
        let h_old = SelectObject(hdc_mem, hbm_mem.into());
        let success = PrintWindow(hwnd, hdc_mem, 2);

        let mut result = None;
        if success != 0 {
            let mut bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width,
                    biHeight: -height,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    biSizeImage: 0,
                    biXPelsPerMeter: 0,
                    biYPelsPerMeter: 0,
                    biClrUsed: 0,
                    biClrImportant: 0,
                },
                bmiColors: [windows::Win32::Graphics::Gdi::RGBQUAD::default(); 1],
            };
            let mut pixels = vec![0u8; (width * height * 4) as usize];
            if GetDIBits(hdc_mem, hbm_mem, 0, height as u32, Some(pixels.as_mut_ptr() as *mut _), &mut bmi, DIB_RGB_COLORS) != 0 {
                for chunk in pixels.chunks_exact_mut(4) {
                    let b = chunk[0];
                    let r = chunk[2];
                    chunk[0] = r;
                    chunk[2] = b;
                    chunk[3] = 255;
                }
                if let Ok(Some(png_base64)) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    if let Some(mut img) = image::RgbaImage::from_raw(width as u32, height as u32, pixels) {
                        if img.width() > max_width || img.height() > max_height {
                            let dyn_img = image::DynamicImage::ImageRgba8(img);
                            img = dyn_img.resize(max_width, max_height, image::imageops::FilterType::Triangle).into_rgba8();
                        }
                        let mut buf = std::io::Cursor::new(Vec::new());
                        if image::write_buffer_with_format(&mut buf, &img, img.width(), img.height(), image::ColorType::Rgba8, image::ImageFormat::Png).is_ok() {
                            use base64::Engine;
                            let b64 = base64::engine::general_purpose::STANDARD.encode(buf.into_inner());
                            return Some(format!("data:image/png;base64,{}", b64));
                        }
                    }
                    None
                })) {
                    result = Some(png_base64);
                }
            }
        }
        SelectObject(hdc_mem, h_old);
        let _ = DeleteObject(hbm_mem.into());
        let _ = DeleteDC(hdc_mem);
        ReleaseDC(None, hdc_screen);

        result
    }
}
