//! Background workers for the Linux backend: pointer hit-testing (click-through),
//! window list / focus tracking, volume and brightness watchers, the settings file
//! watcher, and window placement for the notch, dock and overlay.

use crate::state::*;
use crate::types::{BrightnessChangeEvent, IntRect, VolumeChangeEvent};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, Ordering};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};

// ---------------------------------------------------------------- window rects

/// Physical outer rect (x, y, w, h) of Nectar's own windows, kept fresh from window events.
static WINDOW_RECTS: Mutex<Option<HashMap<String, (i32, i32, i32, i32)>>> = Mutex::new(None);

pub fn window_rect(label: &str) -> Option<(i32, i32, i32, i32)> {
    WINDOW_RECTS.lock().ok()?.as_ref()?.get(label).copied()
}

pub fn refresh_window_rect(win: &tauri::WebviewWindow) {
    if let (Ok(p), Ok(s)) = (win.outer_position(), win.outer_size()) {
        if let Ok(mut g) = WINDOW_RECTS.lock() {
            g.get_or_insert_with(HashMap::new)
                .insert(win.label().to_string(), (p.x, p.y, s.width as i32, s.height as i32));
        }
    }
}

pub fn wire_window_events(app: &AppHandle, window: &tauri::WebviewWindow) {
    refresh_window_rect(window);
    let win = window.clone();
    let app = app.clone();
    window.on_window_event(move |e| match e {
        tauri::WindowEvent::Moved(_) | tauri::WindowEvent::Resized(_) | tauri::WindowEvent::ScaleFactorChanged { .. } => {
            refresh_window_rect(&win);
        }
        tauri::WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            crate::commands::restore_and_exit(&app);
        }
        _ => {}
    });
}

// ---------------------------------------------------------------- placement

/// A monitor in physical pixels, with the same accessors as tauri::Monitor.
#[derive(Clone, Debug)]
pub struct Mon {
    name: Option<String>,
    pos: tauri::PhysicalPosition<i32>,
    size: tauri::PhysicalSize<u32>,
    scale: f64,
    pub primary: bool,
}

impl Mon {
    pub fn name(&self) -> Option<&String> {
        self.name.as_ref()
    }
    pub fn position(&self) -> &tauri::PhysicalPosition<i32> {
        &self.pos
    }
    pub fn size(&self) -> &tauri::PhysicalSize<u32> {
        &self.size
    }
    pub fn scale_factor(&self) -> f64 {
        self.scale
    }
}

/// UI scale, read once on the main thread at startup (X11 has one scale for all monitors).
static SCALE_BITS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0x3FF0000000000000); // 1.0

pub fn set_scale(scale: f64) {
    if scale.is_finite() && scale > 0.0 {
        SCALE_BITS.store(scale.to_bits(), Ordering::Relaxed);
    }
}

fn scale() -> f64 {
    f64::from_bits(SCALE_BITS.load(Ordering::Relaxed))
}

/// All monitors. Safe from any thread (no GDK), see x11::monitors.
pub fn monitors() -> Vec<Mon> {
    crate::x11::monitors()
        .into_iter()
        .map(|m| Mon {
            name: (!m.name.is_empty()).then_some(m.name),
            pos: tauri::PhysicalPosition::new(m.x, m.y),
            size: tauri::PhysicalSize::new(m.width, m.height),
            scale: scale(),
            primary: m.primary,
        })
        .collect()
}

pub fn primary_monitor(_app: &AppHandle) -> Option<Mon> {
    let list = monitors();
    list.iter().find(|m| m.primary).cloned().or_else(|| list.into_iter().next())
}

pub fn monitor_for(app: &AppHandle, kind: &str) -> Option<Mon> {
    let key = if kind == "dock" { "nectar-dock-monitor-id" } else { "nectar-notch-monitor-id" };
    let mode_key = if kind == "dock" { "nectar-dock-monitor-mode" } else { "nectar-notch-monitor-mode" };
    if crate::utils::setting_str(mode_key).as_deref() == Some("specific") {
        if let Some(id) = crate::utils::setting_str(key) {
            if let Some(m) = monitors().into_iter().find(|m| monitor_id(m) == id) {
                return Some(m);
            }
        }
    }
    primary_monitor(app)
}

pub fn monitor_id(m: &Mon) -> String {
    m.name().cloned().unwrap_or_else(|| format!("{}x{}", m.position().x, m.position().y))
}

/// Shows a panel window click-through; the pointer loop makes parts clickable later.
fn show_click_through(win: &tauri::WebviewWindow) {
    let _ = win.show();
    let _ = win.set_ignore_cursor_events(true);
    if let Ok(mut g) = LAST_IGNORE.lock() {
        if let Some(m) = g.as_mut() {
            m.retain(|k, _| *k != win.label());
        }
    }
}

pub fn position_notch(app: &AppHandle) {
    let Some(win) = app.get_webview_window("main") else { return };
    let Some(m) = monitor_for(app, "notch") else { return };
    let (pos, size, scale) = (*m.position(), *m.size(), m.scale_factor());
    let height = (420.0 * crate::utils::get_nectar_scale(app) * scale) as u32;
    let _ = win.set_size(tauri::PhysicalSize::new(size.width, height));
    let _ = win.set_position(tauri::PhysicalPosition::new(pos.x, pos.y));
    let _ = win.set_always_on_top(true);
    show_click_through(&win);
    refresh_window_rect(&win);
}

/// Places the dock along the bottom of its monitor. In "fixed" mode a strut keeps
/// maximized windows above it, like the Windows appbar.
pub fn position_dock(app: &AppHandle, mode: &str) {
    let Some(win) = app.get_webview_window("dock") else { return };
    let Some(m) = monitor_for(app, "dock") else { return };
    let (pos, size, scale) = (*m.position(), *m.size(), m.scale_factor());
    let height = win.outer_size().map(|s| s.height).unwrap_or(0);
    let height = if height <= 10 { (600.0 * scale) as u32 } else { height };
    let _ = win.set_size(tauri::PhysicalSize::new(size.width, height));
    let _ = win.set_position(tauri::PhysicalPosition::new(pos.x, pos.y + size.height as i32 - height as i32));
    let _ = win.set_always_on_top(true);
    show_click_through(&win);
    refresh_window_rect(&win);

    let reserve = if mode == "fixed" { (56.0 * crate::utils::get_nectar_scale(app) * scale) as u32 } else { 0 };
    let (start_x, end_x) = (pos.x.max(0) as u32, (pos.x + size.width as i32 - 1).max(0) as u32);
    // Strut is relative to the whole X screen, so the bottom value counts from its edge.
    let screen_h = crate::x11::x().map(|x| x.height as i32).unwrap_or(size.height as i32);
    let from_bottom = (screen_h - (pos.y + size.height as i32)).max(0) as u32;
    std::thread::spawn(move || {
        // The X window may not be mapped yet right after show().
        for _ in 0..20 {
            if let Some(xid) = crate::x11::find_own_window("Nectar Dock") {
                if reserve > 0 {
                    crate::x11::set_strut(xid, 0, reserve + from_bottom, start_x, end_x);
                } else {
                    crate::x11::clear_strut(xid);
                }
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(150));
        }
    });
}

pub fn sync_overlay(app: &AppHandle) {
    if OVERLAY_IN_SPLASH.load(Ordering::Relaxed) {
        return;
    }
    let Some(win) = app.get_webview_window("overlay") else { return };
    let Some(m) = primary_monitor(app) else { return };
    let _ = win.set_position(*m.position());
    let _ = win.set_size(*m.size());
    refresh_window_rect(&win);
}

// ---------------------------------------------------------------- hit testing

static LAST_IGNORE: Mutex<Option<HashMap<&'static str, bool>>> = Mutex::new(None);
static LAST_EMIT: Mutex<Option<HashMap<&'static str, bool>>> = Mutex::new(None);
static DOCK_EXPIRY: AtomicI64 = AtomicI64::new(0);
static NOTCH_EXPIRY: AtomicI64 = AtomicI64::new(0);
static TOP_EDGE_ENTER: AtomicI64 = AtomicI64::new(0);
static TOP_EDGE_ARMED: AtomicBool = AtomicBool::new(false);
static LEFT_ENTER: AtomicI64 = AtomicI64::new(0);
static RIGHT_ENTER: AtomicI64 = AtomicI64::new(0);
static VOLUME_EXPIRY: AtomicI64 = AtomicI64::new(0);
static BRIGHTNESS_EXPIRY: AtomicI64 = AtomicI64::new(0);

fn set_ignore(app: &AppHandle, label: &'static str, ignore: bool) {
    let mut g = LAST_IGNORE.lock().unwrap_or_else(|e| e.into_inner());
    let map = g.get_or_insert_with(HashMap::new);
    if map.get(label) == Some(&ignore) {
        return;
    }
    if let Some(w) = app.get_webview_window(label) {
        // GTK has no input shape for a window that was never shown (tao panics on it).
        if !w.is_visible().unwrap_or(false) {
            return;
        }
        if w.set_ignore_cursor_events(ignore).is_ok() {
            if std::env::var_os("NECTAR_HIT_DEBUG").is_some() {
                crate::diagnostics::log(&format!("hit: {label} click-through={ignore} dock_rect={:?} notch_rect={:?} win={:?}",
                    DOCK_RECT.lock().ok().and_then(|g| *g), NOTCH_RECT.lock().ok().and_then(|g| *g), window_rect(label)));
            }
            map.insert(label, ignore);
        }
    }
}

fn emit_changed(app: &AppHandle, key: &'static str, target: Option<&str>, value: bool) {
    let mut g = LAST_EMIT.lock().unwrap_or_else(|e| e.into_inner());
    let map = g.get_or_insert_with(HashMap::new);
    if map.get(key) == Some(&value) {
        return;
    }
    map.insert(key, value);
    let _ = match target {
        Some(t) => app.emit_to(t, key, value),
        None => app.emit(key, value),
    };
}

fn in_rect(x: i32, y: i32, (rx, ry, rw, rh): (i32, i32, i32, i32)) -> bool {
    x >= rx && x <= rx + rw && y >= ry && y <= ry + rh
}

fn edge_dwell_met(entered: &AtomicI64, inside: bool, now: i64, dwell: i64) -> bool {
    if !inside {
        entered.store(0, Ordering::Relaxed);
        return false;
    }
    let since = entered.load(Ordering::Relaxed);
    if since == 0 {
        entered.store(now, Ordering::Relaxed);
    }
    dwell <= 0 || (since != 0 && now - since >= dwell)
}

fn css_rect_to_physical(win_pos: (i32, i32), r: IntRect, scale: f64) -> (i32, i32, i32, i32) {
    (
        win_pos.0 + (r.x as f64 * scale) as i32,
        win_pos.1 + (r.y as f64 * scale) as i32,
        (r.width as f64 * scale) as i32,
        (r.height as f64 * scale) as i32,
    )
}

fn mode(kind: &str) -> String {
    let (key, default) = if kind == "dock" { ("nectar-dock-mode", "smart") } else { ("nectar-notch-mode", "fixed") };
    crate::utils::setting_str(key).unwrap_or_else(|| default.into())
}

fn process_pointer(app: &AppHandle, cx: i32, cy: i32, mask: u16, now: i64, mon: (i32, i32, i32, i32), scale: f64) {
    let (mx, my, mw, mh) = mon;
    let fullscreen = CURRENT_FOREGROUND_FULLSCREEN.load(Ordering::Relaxed);
    let menu_open = MENU_IS_OPEN.load(Ordering::Relaxed);
    let left_held = crate::x11::left_button_held(mask);

    // --- Dock
    if let Some(_dock_win) = app.get_webview_window("dock").filter(|w| w.is_visible().unwrap_or(false)) {
        let mut interactive = false;
        let mut span: Option<(i32, i32)> = None;
        if let Some((wx, wy, ww, wh)) = window_rect("dock") {
            if in_rect(cx, cy, (wx, wy, ww, wh)) {
                if let Some(r) = *DOCK_RECT.lock().unwrap_or_else(|e| e.into_inner()) {
                    let (rx, ry, rw, rh) = css_rect_to_physical((wx, wy), r, scale);
                    let hyst = if LAST_IGNORE.lock().ok().and_then(|g| g.as_ref().and_then(|m| m.get("dock").copied())) == Some(false) { (10.0 * scale) as i32 } else { 0 };
                    let (px, pt, pb) = ((5.0 * scale) as i32, (8.0 * scale) as i32, (5.0 * scale) as i32);
                    let bx = (rx - px - hyst, ry - pt - hyst, rw + 2 * px + 2 * hyst, rh + pt + pb + 2 * hyst);
                    interactive = in_rect(cx, cy, bx);
                    span = Some((bx.0, bx.0 + bx.2));
                }
                if !interactive && menu_open {
                    if let Some(r) = *MENU_RECT.lock().unwrap_or_else(|e| e.into_inner()) {
                        let (rx, ry, rw, rh) = css_rect_to_physical((wx, wy), r, scale);
                        let pad = (5.0 * scale) as i32;
                        interactive = in_rect(cx, cy, (rx - pad, ry - pad, rw + 2 * pad, rh + 2 * pad));
                    }
                }
            }
        }
        let narrow = crate::utils::setting_str("nectar-dock-edge-area").as_deref() == Some("narrow");
        let pad = (60.0 * scale) as i32;
        let over_span = !narrow || span.is_none_or(|(l, r)| cx >= l - pad && cx <= r + pad);
        let at_bottom = cy >= my + mh - (8.0 * scale) as i32 && cx >= mx && cx <= mx + mw && over_span;
        if fullscreen {
            DOCK_EXPIRY.store(0, Ordering::Relaxed);
        }
        if !fullscreen && (at_bottom || DOCK_HOVERED.load(Ordering::Relaxed)) {
            DOCK_EXPIRY.store(now + 500, Ordering::Relaxed);
        }
        if at_bottom && !fullscreen {
            if let Some((l, r)) = span {
                if cx >= l - pad && cx <= r + pad {
                    interactive = true;
                }
            }
        }
        emit_changed(app, "dock-edge-hover", Some("dock"), now < DOCK_EXPIRY.load(Ordering::Relaxed));
        let ignore = fullscreen || (!interactive && !menu_open && !DOCK_IS_DRAGGING.load(Ordering::Relaxed));
        if std::env::var_os("NECTAR_HIT_DEBUG").is_some() {
            crate::diagnostics::log(&format!("dock cursor=({cx},{cy}) interactive={interactive} span={span:?} fs={fullscreen} ignore={ignore}"));
        }
        set_ignore(app, "dock", ignore);
    } else {
        emit_changed(app, "dock-edge-hover", Some("dock"), false);
        set_ignore(app, "dock", true);
    }

    // --- Notch
    if let Some(notch_win) = app.get_webview_window("main").filter(|w| w.is_visible().unwrap_or(false)) {
        let notch_mode = mode("notch");
        let suppressed = fullscreen && notch_mode != "fixed";
        let hovered = !suppressed && NOTCH_HOVERED.load(Ordering::Relaxed);
        let narrow = crate::utils::setting_str("nectar-notch-edge-area").as_deref() == Some("narrow");
        let rect = window_rect("main").and_then(|(wx, wy, _, _)| {
            NOTCH_RECT.lock().ok().and_then(|g| *g).map(|r| ((wx, wy), r))
        });
        let pad = (60.0 * scale) as i32;
        let span = rect.map(|((wx, _), r)| {
            let l = wx + (r.x as f64 * scale) as i32;
            (l, l + (r.width as f64 * scale) as i32)
        });
        let over_span = !narrow || span.is_none_or(|(l, r)| cx >= l - pad && cx <= r + pad);
        let notch_mon = monitor_for(app, "notch")
            .map(|m| (m.position().x, m.position().y, m.size().width as i32))
            .unwrap_or((mx, my, mw));
        let in_band = cy <= notch_mon.1 + (8.0 * scale) as i32 && cx >= notch_mon.0 && cx <= notch_mon.0 + notch_mon.2 && over_span;

        // Top edge needs a short dwell so flicking the pointer to a tab bar doesn't pop the notch.
        let armed = if !in_band || suppressed || notch_mode == "fixed" {
            TOP_EDGE_ENTER.store(0, Ordering::Relaxed);
            TOP_EDGE_ARMED.store(false, Ordering::Relaxed);
            false
        } else if TOP_EDGE_ARMED.load(Ordering::Relaxed) {
            true
        } else {
            let dwell = crate::utils::setting_str("nectar-notch-edge-delay").and_then(|v| v.parse::<i64>().ok()).unwrap_or(150).clamp(0, 2000);
            let entered = TOP_EDGE_ENTER.load(Ordering::Relaxed);
            if dwell == 0 || (entered != 0 && now - entered >= dwell) {
                TOP_EDGE_ARMED.store(true, Ordering::Relaxed);
                true
            } else {
                if entered == 0 {
                    TOP_EDGE_ENTER.store(now, Ordering::Relaxed);
                }
                false
            }
        };
        if suppressed {
            NOTCH_EXPIRY.store(0, Ordering::Relaxed);
        }
        if armed || hovered {
            NOTCH_EXPIRY.store(now + 500, Ordering::Relaxed);
        }
        let hover_active = now < NOTCH_EXPIRY.load(Ordering::Relaxed);
        let mut interactive = false;
        if let Some(((wx, wy), r)) = rect {
            let hyst = if LAST_IGNORE.lock().ok().and_then(|g| g.as_ref().and_then(|m| m.get("main").copied())) == Some(false) { (10.0 * scale) as i32 } else { 0 };
            let px = (20.0 * scale) as i32;
            let rx = wx + (r.x as f64 * scale) as i32 - px - hyst;
            let rw = (r.width as f64 * scale) as i32 + 2 * px + 2 * hyst;
            let bottom = wy + (r.height as f64 * scale) as i32 + (5.0 * scale) as i32 + hyst;
            let hidden = NOTCH_HIDDEN.load(Ordering::Relaxed);
            let over_box = cx >= rx && cx <= rx + rw && cy >= wy && cy <= bottom;
            if (!hidden || hover_active) && over_box {
                interactive = true;
            }
            if !fullscreen && in_band && (!hidden || hover_active) && cx >= rx - pad && cx <= rx + rw + pad {
                interactive = true;
            }
            if left_held && (over_box || (in_band && cx >= rx - pad && cx <= rx + rw + pad)) {
                interactive = true;
            }
        }
        emit_changed(app, "notch-edge-hover", Some("main"), hover_active);
        set_ignore(app, "main", suppressed || (!interactive && !menu_open));
        let _ = notch_win;
    } else {
        emit_changed(app, "notch-edge-hover", Some("main"), false);
        set_ignore(app, "main", true);
    }

    // --- Volume / brightness edge HUDs on the overlay
    let mixer = VOLUME_MIXER_RECT.lock().ok().and_then(|g| *g).map(|(x, y, w, h)| {
        (mx + (x * scale) as i32, my + (y * scale) as i32, (w * scale) as i32, (h * scale) as i32)
    });
    let in_mixer = mixer.is_some_and(|r| in_rect(cx, cy, r));
    let ns = crate::utils::get_nectar_scale(app) * scale;
    let (cw, ch) = ((42.0 * ns) as i32, (196.0 * ns) as i32);
    let card = |right: bool| (if right { mx + mw - cw } else { mx }, my + mh / 2 - ch / 2, cw, ch);
    let side_right = |key: &str, default: &str| crate::utils::setting_str(key).unwrap_or_else(|| default.into()) == "right";
    let volume_right = side_right("nectar-volume-side", "left");
    let brightness_right = side_right("nectar-brightness-side", "right");
    if !fullscreen {
        let dwell = crate::utils::setting_str("nectar-edge-delay").and_then(|v| v.parse::<i64>().ok()).unwrap_or(300).clamp(0, 3000);
        let at_edge = |right: bool| cy >= my && cy <= my + mh && if right { cx >= mx + mw - 8 && cx <= mx + mw } else { cx >= mx && cx <= mx + 8 };
        let vol_enter = if volume_right { &RIGHT_ENTER } else { &LEFT_ENTER };
        let over_card = now < VOLUME_EXPIRY.load(Ordering::Relaxed) && in_rect(cx, cy, card(volume_right));
        if edge_dwell_met(vol_enter, at_edge(volume_right), now, dwell) || in_mixer || over_card {
            VOLUME_EXPIRY.store(now + 500, Ordering::Relaxed);
        }
        emit_changed(app, "volume-edge-hover", None, now < VOLUME_EXPIRY.load(Ordering::Relaxed));
        let br_enter = if brightness_right { &RIGHT_ENTER } else { &LEFT_ENTER };
        let br_band = at_edge(brightness_right) && brightness_right != volume_right && crate::system::get_brightness_cached().is_some();
        if edge_dwell_met(br_enter, br_band, now, dwell) {
            BRIGHTNESS_EXPIRY.store(now + 500, Ordering::Relaxed);
        }
        emit_changed(app, "brightness-edge-hover", None, now < BRIGHTNESS_EXPIRY.load(Ordering::Relaxed));
        let over_cards = in_mixer || in_rect(cx, cy, card(false)) || in_rect(cx, cy, card(true));
        set_ignore(app, "overlay", !over_cards);
    } else {
        emit_changed(app, "volume-edge-hover", None, false);
        emit_changed(app, "brightness-edge-hover", None, false);
        set_ignore(app, "overlay", true);
    }
}

/// Polls the pointer (X11 has no global mouse hook without grabs) and updates click-through.
pub fn start_pointer_loop(app: AppHandle) {
    std::thread::spawn(move || {
        let mut last = (i32::MIN, i32::MIN, 0u16);
        let mut last_process = 0i64;
        let mut mon_cache: Option<((i32, i32, i32, i32), f64, i64)> = None;
        loop {
            if SHUTTING_DOWN.load(Ordering::Relaxed) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(16));
            let Some((cx, cy, mask)) = crate::x11::query_pointer() else {
                std::thread::sleep(std::time::Duration::from_millis(500));
                continue;
            };
            let now = crate::utils::get_now_ms();
            // Moving pointer: every tick. Still pointer: a few times a second for dwell timers.
            if (cx, cy, mask) == last && now - last_process < 120 {
                continue;
            }
            last = (cx, cy, mask);
            last_process = now;
            if mon_cache.is_none_or(|(_, _, at)| now - at > 1000) {
                if let Some(m) = monitor_for(&app, "dock") {
                    mon_cache = Some(((m.position().x, m.position().y, m.size().width as i32, m.size().height as i32), m.scale_factor(), now));
                }
            }
            let (mon, scale, _) = mon_cache.unwrap_or(((0, 0, 1920, 1080), 1.0, now));
            process_pointer(&app, cx, cy, mask, now, mon, scale);
        }
    });
}

// ---------------------------------------------------------------- windows and focus

static WINDOWS_DIRTY: AtomicBool = AtomicBool::new(false);
pub static LAST_ACTIVE: AtomicU32 = AtomicU32::new(0);
/// The window that was in front before the last one, used to "send behind".
pub static PREVIOUS_ACTIVE: AtomicU32 = AtomicU32::new(0);

fn own_window(win: u32) -> bool {
    crate::x11::window_pid(win) == Some(std::process::id())
}

fn update_foreground_state(app: &AppHandle) {
    let active = crate::x11::active_window().filter(|&w| !own_window(w));
    let (mut fullscreen, mut maximized, mut dock_overlap, mut notch_overlap) = (false, false, false, false);
    if let Some(w) = active {
        fullscreen = crate::x11::is_fullscreen(w);
        maximized = crate::x11::is_maximized(w);
        if let Some(r) = crate::x11::window_rect(w) {
            let mon = monitor_for(app, "dock");
            if let (Some(m), false) = (mon, crate::x11::is_minimized(w)) {
                let scale = m.scale_factor();
                if let Some((wx, wy, _, _)) = window_rect("dock") {
                    let rect = DOCK_OVERLAP_RECT.lock().ok().and_then(|g| *g).or_else(|| DOCK_RECT.lock().ok().and_then(|g| *g));
                    if let Some(d) = rect {
                        let (dx, dy, dw, dh) = css_rect_to_physical((wx, wy), d, scale);
                        dock_overlap = r.0 < dx + dw && r.0 + r.2 > dx && r.1 < dy + dh && r.1 + r.3 > dy;
                    }
                }
                if let Some((wx, wy, _, _)) = window_rect("main") {
                    if let Some(n) = NOTCH_RECT.lock().ok().and_then(|g| *g) {
                        let (nx, ny, nw, nh) = css_rect_to_physical((wx, wy), n, scale);
                        notch_overlap = r.0 < nx + nw && r.0 + r.2 > nx && r.1 < ny + nh && r.1 + r.3 > ny;
                    }
                }
            }
        }
    }
    CURRENT_FOREGROUND_FULLSCREEN.store(fullscreen, Ordering::Relaxed);
    CURRENT_FOREGROUND_MAXIMIZED.store(maximized, Ordering::Relaxed);
    emit_changed(app, "dock-fullscreen", Some("dock"), fullscreen);
    emit_changed(app, "notch-fullscreen", Some("main"), fullscreen);
    emit_changed(app, "dock-maximized", Some("dock"), maximized);
    emit_changed(app, "dock-overlap", Some("dock"), dock_overlap || fullscreen);
    emit_changed(app, "notch-overlap", Some("main"), notch_overlap || fullscreen);
}

/// Re-sends the current overlap state, e.g. after the dock was re-initialized.
pub fn reset_overlap_state(app: &AppHandle) {
    if let Ok(mut g) = LAST_EMIT.lock() {
        if let Some(m) = g.as_mut() {
            for k in ["dock-overlap", "dock-maximized", "dock-fullscreen", "notch-overlap", "notch-fullscreen"] {
                m.remove(k);
            }
        }
    }
    update_foreground_state(app);
}

fn record_focus(win: u32) {
    if win == 0 || own_window(win) {
        return;
    }
    let prev = LAST_ACTIVE.swap(win, Ordering::Relaxed);
    if prev != win && prev != 0 {
        PREVIOUS_ACTIVE.store(prev, Ordering::Relaxed);
    }
    if let Ok(mut m) = focus_timestamps().lock() {
        m.insert(win as isize, crate::utils::get_now_ms());
    }
    LAST_FOCUS_CHANGE_MS.store(crate::utils::get_now_ms(), Ordering::Relaxed);
}

pub fn start_window_watcher(app: AppHandle) {
    if let Some(w) = crate::x11::active_window() {
        record_focus(w);
    }
    let app_events = app.clone();
    crate::x11::watch_root(move |active_changed| {
        if active_changed {
            if let Some(w) = crate::x11::active_window() {
                record_focus(w);
            }
            update_foreground_state(&app_events);
        }
        WINDOWS_DIRTY.store(true, Ordering::Relaxed);
    });
    // Debounce "windows-changed" and refresh foreground geometry (windows move and maximize
    // without a focus change).
    std::thread::spawn(move || {
        let mut tick = 0u32;
        let mut last_titles = String::new();
        loop {
            if SHUTTING_DOWN.load(Ordering::Relaxed) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(150));
            tick += 1;
            if WINDOWS_DIRTY.swap(false, Ordering::Relaxed) {
                let _ = app.emit("windows-changed", ());
            }
            if tick % 4 == 0 {
                update_foreground_state(&app);
            }
            // Titles change without a client list change (browser tabs), check every 1.5 s.
            if tick % 10 == 0 {
                let titles: String = crate::x11::client_list().iter().map(|&w| crate::x11::window_title(w)).collect::<Vec<_>>().join("\u{1}");
                if titles != last_titles {
                    last_titles = titles;
                    let _ = app.emit("windows-changed", ());
                }
            }
        }
    });
}

// ---------------------------------------------------------------- volume / brightness

pub static CURRENT_VOLUME: Mutex<Option<(f32, bool)>> = Mutex::new(None);

pub fn start_volume_watcher(app: AppHandle) {
    std::thread::spawn(move || loop {
        if SHUTTING_DOWN.load(Ordering::Relaxed) {
            return;
        }
        if let Some(state) = crate::system::volume_state() {
            let changed = {
                let mut g = CURRENT_VOLUME.lock().unwrap_or_else(|e| e.into_inner());
                let changed = g.is_none_or(|(v, m)| (v - state.0).abs() > 0.001 || m != state.1);
                *g = Some(state);
                changed
            };
            if changed {
                let _ = app.emit("volume-change", VolumeChangeEvent { volume: state.0, is_muted: state.1 });
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(400));
    });
}

pub fn start_brightness_watcher(app: AppHandle) {
    std::thread::spawn(move || {
        let mut last: Option<u32> = None;
        loop {
            if SHUTTING_DOWN.load(Ordering::Relaxed) {
                return;
            }
            let b = crate::system::get_brightness();
            crate::system::set_brightness_cache(b);
            if b.is_some() && b != last {
                if last.is_some() {
                    let _ = app.emit("brightness-change", BrightnessChangeEvent { brightness: b.unwrap_or(0) });
                }
                last = b;
            }
            if b.is_none() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(1000));
        }
    });
}

// ---------------------------------------------------------------- settings file

pub static SELF_WRITE_MS: AtomicI64 = AtomicI64::new(0);

/// Picks up edits to settings.json made outside Nectar (e.g. by hand) every second.
pub fn start_settings_watcher(app: AppHandle) {
    let Some(path) = crate::utils::settings_path(&app) else { return };
    std::thread::spawn(move || {
        let mtime = |p: &std::path::Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
        let mut last = mtime(&path);
        loop {
            if SHUTTING_DOWN.load(Ordering::Relaxed) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(1000));
            let now = mtime(&path);
            if now == last {
                continue;
            }
            last = now;
            if crate::utils::get_now_ms() - SELF_WRITE_MS.load(Ordering::Relaxed) < 1500 {
                continue;
            }
            let fresh = crate::utils::read_settings_file(&app);
            let old: HashMap<String, serde_json::Value> =
                SETTINGS_CACHE.get().and_then(|c| c.lock().ok().map(|g| g.clone())).unwrap_or_default();
            crate::utils::replace_settings_cache(fresh.clone());
            for (k, v) in &fresh {
                if old.get(k) != Some(v) {
                    let _ = app.emit("settings-external-changed", serde_json::json!({ "key": k, "value": v }));
                }
            }
        }
    });
}
