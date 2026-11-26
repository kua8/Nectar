//! X11 / EWMH helpers: window list, activation, minimize, close, icons,
//! thumbnails, pointer position, synthetic key presses and struts.
//!
//! Window ids are X11 window ids; the frontend treats them as opaque `hwnd` numbers.

use std::sync::OnceLock;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    self, Atom, AtomEnum, ClientMessageEvent, ConnectionExt as _, EventMask, ImageFormat, PropMode, Window,
};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

pub struct Atoms {
    pub net_client_list: Atom,
    pub net_client_list_stacking: Atom,
    pub net_active_window: Atom,
    pub net_wm_name: Atom,
    pub utf8_string: Atom,
    pub net_wm_pid: Atom,
    pub net_wm_icon: Atom,
    pub net_wm_state: Atom,
    pub net_wm_state_hidden: Atom,
    pub net_wm_state_fullscreen: Atom,
    pub net_wm_state_max_vert: Atom,
    pub net_wm_state_max_horz: Atom,
    pub net_wm_state_skip_taskbar: Atom,
    pub net_wm_state_above: Atom,
    pub net_wm_window_type: Atom,
    pub net_wm_window_type_normal: Atom,
    pub net_wm_window_type_dialog: Atom,
    pub net_wm_window_type_dock: Atom,
    pub net_wm_window_type_desktop: Atom,
    pub net_wm_window_type_utility: Atom,
    pub net_close_window: Atom,
    pub net_wm_strut: Atom,
    pub net_wm_strut_partial: Atom,
    pub net_frame_extents: Atom,
    pub net_wm_desktop: Atom,
    pub net_current_desktop: Atom,
    pub net_showing_desktop: Atom,
    pub wm_change_state: Atom,
    pub wm_transient_for: Atom,
    pub gtk_application_id: Atom,
}

pub struct X {
    pub conn: RustConnection,
    pub root: Window,
    pub atoms: Atoms,
    pub width: u16,
    pub height: u16,
}

fn intern(conn: &RustConnection, name: &str) -> Atom {
    conn.intern_atom(false, name.as_bytes())
        .ok()
        .and_then(|c| c.reply().ok())
        .map(|r| r.atom)
        .unwrap_or(0)
}

fn connect() -> Option<X> {
    let (conn, screen_num) = x11rb::connect(None).ok()?;
    let screen = conn.setup().roots.get(screen_num)?.clone();
    let a = |n: &str| intern(&conn, n);
    let atoms = Atoms {
        net_client_list: a("_NET_CLIENT_LIST"),
        net_client_list_stacking: a("_NET_CLIENT_LIST_STACKING"),
        net_active_window: a("_NET_ACTIVE_WINDOW"),
        net_wm_name: a("_NET_WM_NAME"),
        utf8_string: a("UTF8_STRING"),
        net_wm_pid: a("_NET_WM_PID"),
        net_wm_icon: a("_NET_WM_ICON"),
        net_wm_state: a("_NET_WM_STATE"),
        net_wm_state_hidden: a("_NET_WM_STATE_HIDDEN"),
        net_wm_state_fullscreen: a("_NET_WM_STATE_FULLSCREEN"),
        net_wm_state_max_vert: a("_NET_WM_STATE_MAXIMIZED_VERT"),
        net_wm_state_max_horz: a("_NET_WM_STATE_MAXIMIZED_HORZ"),
        net_wm_state_skip_taskbar: a("_NET_WM_STATE_SKIP_TASKBAR"),
        net_wm_state_above: a("_NET_WM_STATE_ABOVE"),
        net_wm_window_type: a("_NET_WM_WINDOW_TYPE"),
        net_wm_window_type_normal: a("_NET_WM_WINDOW_TYPE_NORMAL"),
        net_wm_window_type_dialog: a("_NET_WM_WINDOW_TYPE_DIALOG"),
        net_wm_window_type_dock: a("_NET_WM_WINDOW_TYPE_DOCK"),
        net_wm_window_type_desktop: a("_NET_WM_WINDOW_TYPE_DESKTOP"),
        net_wm_window_type_utility: a("_NET_WM_WINDOW_TYPE_UTILITY"),
        net_close_window: a("_NET_CLOSE_WINDOW"),
        net_wm_strut: a("_NET_WM_STRUT"),
        net_wm_strut_partial: a("_NET_WM_STRUT_PARTIAL"),
        net_frame_extents: a("_NET_FRAME_EXTENTS"),
        net_wm_desktop: a("_NET_WM_DESKTOP"),
        net_current_desktop: a("_NET_CURRENT_DESKTOP"),
        net_showing_desktop: a("_NET_SHOWING_DESKTOP"),
        wm_change_state: a("WM_CHANGE_STATE"),
        wm_transient_for: AtomEnum::WM_TRANSIENT_FOR.into(),
        gtk_application_id: a("_GTK_APPLICATION_ID"),
    };
    Some(X { conn, root: screen.root, atoms, width: screen.width_in_pixels, height: screen.height_in_pixels })
}

/// Shared connection for requests. Returns None when there is no X server (e.g. pure Wayland).
pub fn x() -> Option<&'static X> {
    static X_CONN: OnceLock<Option<X>> = OnceLock::new();
    X_CONN.get_or_init(connect).as_ref()
}

pub fn available() -> bool {
    x().is_some()
}

fn get_property(x: &X, win: Window, prop: Atom, ty: impl Into<Atom>, long_len: u32) -> Option<xproto::GetPropertyReply> {
    if prop == 0 {
        return None;
    }
    x.conn.get_property(false, win, prop, ty, 0, long_len).ok()?.reply().ok()
}

fn prop_u32s(x: &X, win: Window, prop: Atom, ty: impl Into<Atom>) -> Vec<u32> {
    get_property(x, win, prop, ty, 1 << 16)
        .and_then(|r| r.value32().map(|v| v.collect()))
        .unwrap_or_default()
}

pub fn client_list() -> Vec<Window> {
    let Some(x) = x() else { return Vec::new() };
    prop_u32s(x, x.root, x.atoms.net_client_list, AtomEnum::WINDOW)
}

/// Clients bottom to top.
pub fn client_list_stacking() -> Vec<Window> {
    let Some(x) = x() else { return Vec::new() };
    prop_u32s(x, x.root, x.atoms.net_client_list_stacking, AtomEnum::WINDOW)
}

pub fn active_window() -> Option<Window> {
    let x = x()?;
    prop_u32s(x, x.root, x.atoms.net_active_window, AtomEnum::WINDOW).first().copied().filter(|&w| w != 0)
}

pub fn window_pid(win: Window) -> Option<u32> {
    let x = x()?;
    prop_u32s(x, win, x.atoms.net_wm_pid, AtomEnum::CARDINAL).first().copied()
}

pub fn window_title(win: Window) -> String {
    let Some(x) = x() else { return String::new() };
    if let Some(r) = get_property(x, win, x.atoms.net_wm_name, x.atoms.utf8_string, 1024) {
        if !r.value.is_empty() {
            return String::from_utf8_lossy(&r.value).to_string();
        }
    }
    get_property(x, win, AtomEnum::WM_NAME.into(), AtomEnum::ANY, 1024)
        .map(|r| String::from_utf8_lossy(&r.value).to_string())
        .unwrap_or_default()
}

/// WM_CLASS as (instance, class).
pub fn wm_class(win: Window) -> (String, String) {
    let Some(x) = x() else { return Default::default() };
    let Some(r) = get_property(x, win, AtomEnum::WM_CLASS.into(), AtomEnum::STRING, 256) else { return Default::default() };
    let mut parts = r.value.split(|&b| b == 0).map(|s| String::from_utf8_lossy(s).to_string());
    let instance = parts.next().unwrap_or_default();
    let class = parts.next().unwrap_or_default();
    (instance, class)
}

pub fn gtk_application_id(win: Window) -> Option<String> {
    let x = x()?;
    let r = get_property(x, win, x.atoms.gtk_application_id, x.atoms.utf8_string, 256)?;
    (!r.value.is_empty()).then(|| String::from_utf8_lossy(&r.value).to_string())
}

pub fn window_states(win: Window) -> Vec<Atom> {
    let Some(x) = x() else { return Vec::new() };
    prop_u32s(x, win, x.atoms.net_wm_state, AtomEnum::ATOM)
}

fn window_types(win: Window) -> Vec<Atom> {
    let Some(x) = x() else { return Vec::new() };
    prop_u32s(x, win, x.atoms.net_wm_window_type, AtomEnum::ATOM)
}

/// Whether a client belongs in a taskbar: a normal top-level window that doesn't opt out.
pub fn is_taskbar_window(win: Window) -> bool {
    let Some(x) = x() else { return false };
    let a = &x.atoms;
    if window_states(win).contains(&a.net_wm_state_skip_taskbar) {
        return false;
    }
    let types = window_types(win);
    let transient = !prop_u32s(x, win, a.wm_transient_for, AtomEnum::WINDOW).is_empty();
    if types.is_empty() {
        return !transient;
    }
    types.iter().any(|&t| t == a.net_wm_window_type_normal || (t == a.net_wm_window_type_dialog && !transient))
        && !types.iter().any(|&t| t == a.net_wm_window_type_dock || t == a.net_wm_window_type_desktop)
}

pub fn is_minimized(win: Window) -> bool {
    x().is_some_and(|x| window_states(win).contains(&x.atoms.net_wm_state_hidden))
}

pub fn is_fullscreen(win: Window) -> bool {
    x().is_some_and(|x| window_states(win).contains(&x.atoms.net_wm_state_fullscreen))
}

pub fn is_maximized(win: Window) -> bool {
    x().is_some_and(|x| {
        let s = window_states(win);
        s.contains(&x.atoms.net_wm_state_max_vert) && s.contains(&x.atoms.net_wm_state_max_horz)
    })
}

/// Outer rect (including the WM frame) in root coordinates.
pub fn window_rect(win: Window) -> Option<(i32, i32, i32, i32)> {
    let x = x()?;
    let geo = x.conn.get_geometry(win).ok()?.reply().ok()?;
    let tr = x.conn.translate_coordinates(win, x.root, 0, 0).ok()?.reply().ok()?;
    let ext = prop_u32s(x, win, x.atoms.net_frame_extents, AtomEnum::CARDINAL);
    let (l, r, t, b) = if ext.len() == 4 { (ext[0] as i32, ext[1] as i32, ext[2] as i32, ext[3] as i32) } else { (0, 0, 0, 0) };
    Some((tr.dst_x as i32 - l, tr.dst_y as i32 - t, geo.width as i32 + l + r, geo.height as i32 + t + b))
}

pub fn window_exists(win: Window) -> bool {
    x().is_some_and(|x| x.conn.get_window_attributes(win).ok().and_then(|c| c.reply().ok()).is_some())
}

fn send_root_message(x: &X, win: Window, ty: Atom, data: [u32; 5]) {
    let ev = ClientMessageEvent::new(32, win, ty, data);
    let _ = x.conn.send_event(
        false,
        x.root,
        EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
        ev,
    );
    let _ = x.conn.flush();
}

/// Raise and focus a window, restoring it if minimized and switching workspace if needed.
pub fn activate(win: Window) {
    let Some(x) = x() else { return };
    let active = active_window().unwrap_or(0);
    // Source 2 = pager/taskbar, so the WM doesn't apply focus-stealing prevention.
    send_root_message(x, win, x.atoms.net_active_window, [2, 0, active, 0, 0]);
}

pub fn minimize(win: Window) {
    let Some(x) = x() else { return };
    // ICCCM IconicState = 3
    send_root_message(x, win, x.atoms.wm_change_state, [3, 0, 0, 0, 0]);
}

pub fn close(win: Window) {
    let Some(x) = x() else { return };
    send_root_message(x, win, x.atoms.net_close_window, [0, 2, 0, 0, 0]);
}

pub fn toggle_showing_desktop() {
    let Some(x) = x() else { return };
    let showing = prop_u32s(x, x.root, x.atoms.net_showing_desktop, AtomEnum::CARDINAL).first().copied().unwrap_or(0);
    send_root_message(x, x.root, x.atoms.net_showing_desktop, [if showing == 0 { 1 } else { 0 }, 0, 0, 0, 0]);
}

/// Best _NET_WM_ICON entry for the requested size.
pub fn window_icon(win: Window, want: u32) -> Option<image::RgbaImage> {
    let x = x()?;
    let data = prop_u32s(x, win, x.atoms.net_wm_icon, AtomEnum::CARDINAL);
    let mut i = 0usize;
    let mut best: Option<(u32, u32, usize)> = None;
    while i + 2 <= data.len() {
        let (w, h) = (data[i], data[i + 1]);
        let n = (w as usize).saturating_mul(h as usize);
        if w == 0 || h == 0 || i + 2 + n > data.len() {
            break;
        }
        let better = match best {
            None => true,
            Some((bw, _, _)) => {
                // Prefer the smallest icon that is at least `want`, else the largest.
                (w >= want && (bw < want || w < bw)) || (bw < want && w > bw)
            }
        };
        if better {
            best = Some((w, h, i + 2));
        }
        i += 2 + n;
    }
    let (w, h, off) = best?;
    let mut img = image::RgbaImage::new(w, h);
    for (idx, px) in img.pixels_mut().enumerate() {
        let argb = data[off + idx];
        *px = image::Rgba([(argb >> 16) as u8, (argb >> 8) as u8, argb as u8, (argb >> 24) as u8]);
    }
    if w > want * 2 {
        img = image::imageops::resize(&img, want, want * h / w.max(1), image::imageops::FilterType::Lanczos3);
    }
    Some(img)
}

/// Grabs the window's current contents. Works for mapped windows under a compositor.
pub fn capture_window(win: Window, max_w: u32, max_h: u32) -> Option<image::RgbaImage> {
    let x = x()?;
    if is_minimized(win) {
        return None;
    }
    let geo = x.conn.get_geometry(win).ok()?.reply().ok()?;
    if geo.width < 2 || geo.height < 2 {
        return None;
    }
    let reply = x
        .conn
        .get_image(ImageFormat::Z_PIXMAP, win, 0, 0, geo.width, geo.height, !0)
        .ok()?
        .reply()
        .ok()?;
    let (w, h) = (geo.width as u32, geo.height as u32);
    let data = reply.data;
    if data.len() < (w * h * 4) as usize {
        return None;
    }
    let mut img = image::RgbaImage::new(w, h);
    for (idx, px) in img.pixels_mut().enumerate() {
        let o = idx * 4;
        *px = image::Rgba([data[o + 2], data[o + 1], data[o], 255]);
    }
    // A window that hasn't painted comes back black.
    let painted = img.pixels().step_by(97).filter(|p| p[0] != 0 || p[1] != 0 || p[2] != 0).count();
    if painted * 20 < (w * h / 97) as usize {
        return None;
    }
    Some(image::imageops::thumbnail(&img, max_w.min(w).max(1), (max_w.min(w) * h / w).min(max_h).max(1)))
}

/// Pointer position in root coordinates and the button/modifier mask.
pub fn query_pointer() -> Option<(i32, i32, u16)> {
    let x = x()?;
    let r = x.conn.query_pointer(x.root).ok()?.reply().ok()?;
    Some((r.root_x as i32, r.root_y as i32, u16::from(r.mask)))
}

pub fn left_button_held(mask: u16) -> bool {
    mask & u16::from(xproto::KeyButMask::BUTTON1) != 0
}

fn keysym_to_keycode(x: &X, keysym: u32) -> Option<u8> {
    let setup = x.conn.setup();
    let (min, max) = (setup.min_keycode, setup.max_keycode);
    let map = x.conn.get_keyboard_mapping(min, max - min + 1).ok()?.reply().ok()?;
    let per = map.keysyms_per_keycode as usize;
    map.keysyms
        .chunks(per.max(1))
        .position(|syms| syms.contains(&keysym))
        .map(|i| min + i as u8)
}

/// Presses the keys in order and releases them in reverse, like a user chord.
pub fn send_key_chord(keysyms: &[u32]) {
    use x11rb::protocol::xtest::ConnectionExt as _;
    let Some(x) = x() else { return };
    let codes: Vec<u8> = keysyms.iter().filter_map(|&k| keysym_to_keycode(x, k)).collect();
    if codes.len() != keysyms.len() {
        return;
    }
    for &c in &codes {
        let _ = x.conn.xtest_fake_input(xproto::KEY_PRESS_EVENT, c, 0, x.root, 0, 0, 0);
    }
    for &c in codes.iter().rev() {
        let _ = x.conn.xtest_fake_input(xproto::KEY_RELEASE_EVENT, c, 0, x.root, 0, 0, 0);
    }
    let _ = x.conn.flush();
}

pub mod keysym {
    pub const SUPER_L: u32 = 0xffeb;
    pub const CONTROL_L: u32 = 0xffe3;
    pub const ALT_L: u32 = 0xffe9;
    pub const UP: u32 = 0xff52;
    pub const DOWN: u32 = 0xff54;
    pub const D: u32 = 0x0064;
}

/// Finds one of Nectar's own top-level windows by title.
pub fn find_own_window(title: &str) -> Option<Window> {
    let x = x()?;
    let pid = std::process::id();
    let tree = x.conn.query_tree(x.root).ok()?.reply().ok()?;
    // Client windows may be reparented into frames, so look one level down as well.
    let mut candidates: Vec<Window> = tree.children.clone();
    for &c in &tree.children {
        if let Some(t) = x.conn.query_tree(c).ok().and_then(|c| c.reply().ok()) {
            candidates.extend(t.children);
        }
    }
    candidates.into_iter().find(|&w| window_pid(w) == Some(pid) && window_title(w) == title)
}

/// Reserves space at the bottom (or top) of the screen so maximized windows stop at the dock.
pub fn set_strut(win: Window, top: u32, bottom: u32, start_x: u32, end_x: u32) {
    let Some(x) = x() else { return };
    let partial: [u32; 12] = [
        0, 0, top, bottom,
        0, 0, 0, 0,
        if top > 0 { start_x } else { 0 }, if top > 0 { end_x } else { 0 },
        if bottom > 0 { start_x } else { 0 }, if bottom > 0 { end_x } else { 0 },
    ];
    let _ = x.conn.change_property32(PropMode::REPLACE, win, x.atoms.net_wm_strut_partial, AtomEnum::CARDINAL, &partial);
    let _ = x.conn.change_property32(PropMode::REPLACE, win, x.atoms.net_wm_strut, AtomEnum::CARDINAL, &[0, 0, top, bottom]);
    let _ = x.conn.flush();
}

pub fn clear_strut(win: Window) {
    let Some(x) = x() else { return };
    let _ = x.conn.delete_property(win, x.atoms.net_wm_strut_partial);
    let _ = x.conn.delete_property(win, x.atoms.net_wm_strut);
    let _ = x.conn.flush();
}

/// Watches the root window for client list and focus changes on its own connection.
/// `on_change(active_changed)` runs on the watcher thread.
pub fn watch_root(on_change: impl Fn(bool) + Send + 'static) {
    std::thread::spawn(move || {
        let Some(own) = connect() else { return };
        let attrs = xproto::ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE);
        if own.conn.change_window_attributes(own.root, &attrs).is_err() {
            return;
        }
        let _ = own.conn.flush();
        loop {
            let Ok(event) = own.conn.wait_for_event() else { return };
            if let x11rb::protocol::Event::PropertyNotify(ev) = event {
                if ev.atom == own.atoms.net_active_window {
                    on_change(true);
                } else if ev.atom == own.atoms.net_client_list {
                    on_change(false);
                }
            }
        }
    });
}
