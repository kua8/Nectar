use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use serde::{Deserialize, Serialize};
use windows::core::BOOL;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Dwm::{
    DwmIsCompositionEnabled, DwmQueryThumbnailSourceSize, DwmRegisterThumbnail,
    DwmUnregisterThumbnail, DwmUpdateThumbnailProperties, DWM_THUMBNAIL_PROPERTIES,
    DWM_TNP_OPACITY, DWM_TNP_RECTDESTINATION, DWM_TNP_SOURCECLIENTAREAONLY, DWM_TNP_VISIBLE,
};
use windows::Win32::UI::WindowsAndMessaging::{GetAncestor, IsWindow, GA_ROOT};

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct LiveThumbnailTarget {
    pub hwnd: isize,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug)]
struct ThumbnailRecord {
    handle: isize,
    dest_hwnd: isize,
    rect: RECT,
}

// Global registry mapping source_hwnd -> ThumbnailRecord
static ACTIVE_THUMBNAILS: OnceLock<Mutex<HashMap<isize, ThumbnailRecord>>> = OnceLock::new();

fn get_map() -> &'static Mutex<HashMap<isize, ThumbnailRecord>> {
    ACTIVE_THUMBNAILS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Checks if Windows DWM composition is active.
pub fn is_composition_enabled() -> bool {
    unsafe {
        DwmIsCompositionEnabled().map(|b| b.as_bool()).unwrap_or(false)
    }
}

/// Atomically sets the live thumbnail targets for the destination window.
/// Any existing thumbnails not present in `targets` are unregistered.
/// Any existing thumbnails present in `targets` have their position updated.
/// Any new targets are registered and positioned.
pub fn set_live_thumbnails(
    dest_hwnd_val: isize,
    targets: Vec<LiveThumbnailTarget>,
    client_area_only: bool,
) -> Result<Vec<isize>, String> {
    if !is_composition_enabled() {
        return Err("DWM composition is not enabled".to_string());
    }

    let dest_hwnd = HWND(dest_hwnd_val as *mut _);
    unsafe {
        if !IsWindow(Some(dest_hwnd)).as_bool() {
            return Err("Destination window is invalid".to_string());
        }
    }

    let mut map = get_map().lock().map_err(|e| e.to_string())?;
    let target_hwnds: std::collections::HashSet<isize> = targets.iter().map(|t| t.hwnd).collect();

    // 1. Unregister thumbnails no longer in target list
    let to_remove: Vec<isize> = map
        .keys()
        .copied()
        .filter(|k| !target_hwnds.contains(k))
        .collect();

    for src_hwnd_val in to_remove {
        if let Some(record) = map.remove(&src_hwnd_val) {
            unsafe {
                let _ = DwmUnregisterThumbnail(record.handle);
            }
        }
    }

    let mut successfully_active = Vec::new();

    // 2. Register or update each target
    for target in targets {
        let raw_hwnd = HWND(target.hwnd as *mut _);
        let src_hwnd = unsafe {
            if !IsWindow(Some(raw_hwnd)).as_bool() {
                continue;
            }
            let root = GetAncestor(raw_hwnd, GA_ROOT);
            if root.0 != std::ptr::null_mut() && IsWindow(Some(root)).as_bool() {
                root
            } else {
                raw_hwnd
            }
        };

        let rc_dest = RECT {
            left: target.x,
            top: target.y,
            right: target.x + target.width.max(1),
            bottom: target.y + target.height.max(1),
        };

        if let Some(existing) = map.get_mut(&target.hwnd) {
            // Already registered, update destination rect if changed
            if existing.rect != rc_dest || existing.dest_hwnd != dest_hwnd_val {
                existing.dest_hwnd = dest_hwnd_val;
                existing.rect = rc_dest;
                let props = DWM_THUMBNAIL_PROPERTIES {
                    dwFlags: DWM_TNP_RECTDESTINATION | DWM_TNP_VISIBLE | DWM_TNP_OPACITY | DWM_TNP_SOURCECLIENTAREAONLY,
                    rcDestination: rc_dest,
                    rcSource: RECT::default(),
                    opacity: 255,
                    fVisible: BOOL(1),
                    fSourceClientAreaOnly: if client_area_only { BOOL(1) } else { BOOL(0) },
                };
                unsafe {
                    let _ = DwmUpdateThumbnailProperties(existing.handle, &props);
                }
            }
            successfully_active.push(target.hwnd);
        } else {
            // New registration
            unsafe {
                match DwmRegisterThumbnail(dest_hwnd, src_hwnd) {
                    Ok(handle) => {
                        let props = DWM_THUMBNAIL_PROPERTIES {
                            dwFlags: DWM_TNP_RECTDESTINATION | DWM_TNP_VISIBLE | DWM_TNP_OPACITY | DWM_TNP_SOURCECLIENTAREAONLY,
                            rcDestination: rc_dest,
                            rcSource: RECT::default(),
                            opacity: 255,
                            fVisible: BOOL(1),
                            fSourceClientAreaOnly: if client_area_only { BOOL(1) } else { BOOL(0) },
                        };
                        let _ = DwmUpdateThumbnailProperties(handle, &props);
                        map.insert(
                            target.hwnd,
                            ThumbnailRecord {
                                handle,
                                dest_hwnd: dest_hwnd_val,
                                rect: rc_dest,
                            },
                        );
                        successfully_active.push(target.hwnd);
                    }
                    Err(e) => {
                        // Some windows (e.g. elevated or special shell windows) may disallow DWM registration.
                        // In that case, we gracefully continue and let frontend use fallback image.
                        eprintln!("[Nectar DWM] Failed to register thumbnail for HWND {}: {:?}", target.hwnd, e);
                    }
                }
            }
        }
    }

    Ok(successfully_active)
}

/// Unregisters all active thumbnails.
pub fn clear_all_thumbnails() -> Result<(), String> {
    let mut map = get_map().lock().map_err(|e| e.to_string())?;
    for (_, record) in map.drain() {
        unsafe {
            let _ = DwmUnregisterThumbnail(record.handle);
        }
    }
    Ok(())
}

/// Queries the native source window aspect ratio / size.
#[allow(dead_code)]
pub fn query_source_size(src_hwnd: isize) -> Result<(i32, i32), String> {
    let map = get_map().lock().map_err(|e| e.to_string())?;
    if let Some(record) = map.get(&src_hwnd) {
        unsafe {
            match DwmQueryThumbnailSourceSize(record.handle) {
                Ok(size) => Ok((size.cx, size.cy)),
                Err(e) => Err(format!("{:?}", e)),
            }
        }
    } else {
        Err("Thumbnail not registered".to_string())
    }
}
