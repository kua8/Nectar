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

pub fn taskbar_marker_exists() -> bool {
    TASKBAR_MARKER.get().is_some_and(|p| p.exists())
}

fn rect_is_on_screen(r: &windows::Win32::Foundation::RECT) -> bool {
    r.left > -5000 && r.top > -5000
}

unsafe fn monitor_rects() -> Vec<(windows::Win32::Foundation::RECT, bool)> {
    use windows::Win32::Foundation::{LPARAM, RECT};
    use windows::Win32::Graphics::Gdi::{EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO};

    unsafe extern "system" fn collect(hmon: HMONITOR, _: HDC, _: *mut RECT, lparam: LPARAM) -> windows::core::BOOL {
        let list = &mut *(lparam.0 as *mut Vec<(RECT, bool)>);
        let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        if GetMonitorInfoW(hmon, &mut mi).as_bool() {
            list.push((mi.rcMonitor, mi.dwFlags & 1 != 0));
        }
        windows::core::BOOL(1)
    }
    let mut monitors: Vec<(RECT, bool)> = Vec::new();
    let _ = EnumDisplayMonitors(None, None, Some(collect), LPARAM(&mut monitors as *mut _ as isize));
    monitors
}

fn taskbar_monitor(monitors: &[(windows::Win32::Foundation::RECT, bool)], primary: bool, width: i32) -> Option<windows::Win32::Foundation::RECT> {
    if primary {
        monitors.iter().find(|(_, is_primary)| *is_primary).map(|(r, _)| *r)
    } else {
        monitors.iter().find(|(r, is_primary)| !*is_primary && r.right - r.left == width)
            .or_else(|| monitors.iter().find(|(_, is_primary)| !*is_primary))
            .map(|(r, _)| *r)
    }
}

unsafe fn park_below_monitor(hwnd: windows::Win32::Foundation::HWND, primary: bool) {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::UI::WindowsAndMessaging::{GetWindowRect, SetWindowPos, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER};
    let mut cur = RECT::default();
    let _ = GetWindowRect(hwnd, &mut cur);
    let (x, y) = match taskbar_monitor(&monitor_rects(), primary, cur.right - cur.left) {
        Some(m) => (m.left, m.bottom),
        None => (-10000, -10000),
    };
    let _ = SetWindowPos(hwnd, None, x, y, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
}

unsafe fn restore_tray_position(hwnd: windows::Win32::Foundation::HWND, primary: bool, saved: Option<windows::Win32::Foundation::RECT>) {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::UI::WindowsAndMessaging::{GetWindowRect, SetWindowPos, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER};

    if let Some(r) = saved {
        if rect_is_on_screen(&r) {
            let _ = SetWindowPos(hwnd, None, r.left, r.top, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
            return;
        }
    }

    let mut cur = RECT::default();
    let _ = GetWindowRect(hwnd, &mut cur);
    let (w, h) = (cur.right - cur.left, cur.bottom - cur.top);
    let monitors = monitor_rects();
    let inside_a_monitor = monitors.iter().any(|(m, _)| cur.left >= m.left && cur.top >= m.top && cur.right <= m.right && cur.bottom <= m.bottom);
    if inside_a_monitor { return; }

    if let Some(mon) = taskbar_monitor(&monitors, primary, w) {
        let _ = SetWindowPos(hwnd, None, mon.left, mon.bottom - h, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
    }
}

pub fn set_taskbar_visibility(visible: bool, always_on_top: bool) {
    if visible {
        crate::state::NATIVE_TASKBAR_HIDDEN.store(false, std::sync::atomic::Ordering::SeqCst);
    }

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
            if always_on_top {
                2
            } else {
                let orig = ORIGINAL_TASKBAR_STATE.load(std::sync::atomic::Ordering::Relaxed);
                if orig != -1 { orig as isize } else { 1 }
            }
        } else {
            1
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
            use windows::Win32::UI::WindowsAndMessaging::{IsWindowVisible, GetWindowLongA, SetWindowLongA, GWL_EXSTYLE, WS_EX_LAYERED, WS_EX_TRANSPARENT, SetLayeredWindowAttributes, LWA_ALPHA};
            if visible {
                // Revert any lingering WS_EX_LAYERED / WS_EX_TRANSPARENT left by
                // open_system_tray if the user quit before the tray thread cleaned up.
                let ex = GetWindowLongA(tray_hwnd, GWL_EXSTYLE);
                let cleaned = ex & !(WS_EX_LAYERED.0 as i32) & !(WS_EX_TRANSPARENT.0 as i32);
                if cleaned != ex {
                    let _ = SetWindowLongA(tray_hwnd, GWL_EXSTYLE, cleaned);
                    let _ = SetLayeredWindowAttributes(tray_hwnd, windows::Win32::Foundation::COLORREF(0), 255, LWA_ALPHA);
                }
                let saved = ORIGINAL_TRAY_RECT.lock().ok().and_then(|g| *g);
                restore_tray_position(tray_hwnd, true, saved);
                let _ = ShowWindow(tray_hwnd, SW_SHOW);

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
                    if rect_is_on_screen(&rect) && IsWindowVisible(tray_hwnd).as_bool() {
                        if let Ok(mut guard) = ORIGINAL_TRAY_RECT.lock() {
                            *guard = Some(rect);
                        }
                    }
                }
                let _ = ShowWindow(tray_hwnd, SW_HIDE);
                // Move it far off-screen to prevent any "thin line" artifacts or flashes
                park_below_monitor(tray_hwnd, true);
            }
        }

        // 3. Control visibility of secondary taskbars (multi-monitor)
        if let Ok(secondary_tray_hwnd) = FindWindowA(secondary_tray_class, windows::core::PCSTR::null()) {
            use windows::Win32::UI::WindowsAndMessaging::{IsWindowVisible};
            if visible {
                let saved = ORIGINAL_SEC_TRAY_RECT.lock().ok().and_then(|g| *g);
                restore_tray_position(secondary_tray_hwnd, false, saved);
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
                    if rect_is_on_screen(&rect) && IsWindowVisible(secondary_tray_hwnd).as_bool() {
                        if let Ok(mut guard) = ORIGINAL_SEC_TRAY_RECT.lock() {
                            *guard = Some(rect);
                        }
                    }
                }
                let _ = ShowWindow(secondary_tray_hwnd, SW_HIDE);
                park_below_monitor(secondary_tray_hwnd, false);
            }
        }

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

pub fn exe_description(path: &str) -> Option<String> {
    use windows::Win32::Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW};
    static CACHE: OnceLock<std::sync::Mutex<std::collections::HashMap<String, Option<String>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    if let Some(hit) = cache.lock().ok().and_then(|c| c.get(path).cloned()) {
        return hit;
    }

    let result = unsafe {
        let wide = |s: &str| -> Vec<u16> { s.encode_utf16().chain(std::iter::once(0)).collect() };
        let path_w = wide(path);
        let size = GetFileVersionInfoSizeW(windows::core::PCWSTR(path_w.as_ptr()), None);
        if size == 0 {
            None
        } else {
            let mut buf = vec![0u8; size as usize];
            if GetFileVersionInfoW(windows::core::PCWSTR(path_w.as_ptr()), None, size, buf.as_mut_ptr() as *mut _).is_err() {
                None
            } else {
                let query = |sub: &str| -> Option<(*mut std::ffi::c_void, u32)> {
                    let sub_w = wide(sub);
                    let mut ptr: *mut std::ffi::c_void = std::ptr::null_mut();
                    let mut len = 0u32;
                    let ok = VerQueryValueW(buf.as_ptr() as *const _, windows::core::PCWSTR(sub_w.as_ptr()), &mut ptr, &mut len);
                    if ok.as_bool() && !ptr.is_null() && len > 0 { Some((ptr, len)) } else { None }
                };
                query("\\VarFileInfo\\Translation").and_then(|(ptr, _)| {
                    let lang = *(ptr as *const u16);
                    let codepage = *(ptr as *const u16).add(1);
                    let (desc, _) = query(&format!("\\StringFileInfo\\{:04x}{:04x}\\FileDescription", lang, codepage))?;
                    let s = String::from_utf16_lossy(windows::core::PCWSTR(desc as *const u16).as_wide()).trim().to_string();
                    if s.is_empty() { None } else { Some(s) }
                })
            }
        }
    };

    if let Ok(mut c) = cache.lock() { c.insert(path.to_string(), result.clone()); }
    result
}

pub unsafe fn icon_from_absolute_pidl(pidl: *const windows::Win32::UI::Shell::Common::ITEMIDLIST) -> Option<String> {
    use windows::Win32::UI::Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON, SHGFI_PIDL};
    use windows::Win32::UI::WindowsAndMessaging::DestroyIcon;
    let mut shfi: SHFILEINFOW = std::mem::zeroed();
    let res = SHGetFileInfoW(
        windows::core::PCWSTR(pidl as *const u16),
        Default::default(),
        Some(&mut shfi),
        std::mem::size_of::<SHFILEINFOW>() as u32,
        SHGFI_ICON | SHGFI_LARGEICON | SHGFI_PIDL,
    );
    if res != 0 && !shfi.hIcon.is_invalid() {
        let b64 = icon_to_base64(shfi.hIcon);
        let _ = DestroyIcon(shfi.hIcon);
        b64
    } else {
        None
    }
}

pub unsafe fn icon_from_parsing_name(name: &str) -> Option<String> {
    use windows::Win32::System::Com::CoTaskMemFree;
    use windows::Win32::UI::Shell::SHParseDisplayName;
    let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    let mut pidl = std::ptr::null_mut();
    SHParseDisplayName(windows::core::PCWSTR(wide.as_ptr()), None, &mut pidl, 0, None).ok()?;
    if pidl.is_null() { return None; }
    let icon = icon_from_absolute_pidl(pidl);
    CoTaskMemFree(Some(pidl as *const _));
    icon
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

fn settings_cache() -> &'static std::sync::Mutex<std::collections::HashMap<String, serde_json::Value>> {
    crate::state::SETTINGS_CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

/// Load settings.json into the in-memory cache. Call once at startup.
pub fn init_settings_cache(app: &tauri::AppHandle) {
    use tauri::Manager;
    if let Some(path) = app.path().app_config_dir().ok().map(|p| p.join("settings.json")) {
        if let Ok(content) = std::fs::read_to_string(path) {
            if let Ok(settings) = serde_json::from_str::<std::collections::HashMap<String, serde_json::Value>>(&content) {
                if let Ok(mut cache) = settings_cache().lock() {
                    *cache = settings;
                }
            }
        }
    }
}

/// Replace the entire settings cache (used by the file watcher on external changes).
pub fn replace_settings_cache(new_settings: std::collections::HashMap<String, serde_json::Value>) {
    if let Ok(mut cache) = settings_cache().lock() {
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
