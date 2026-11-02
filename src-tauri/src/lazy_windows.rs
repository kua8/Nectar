
use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

pub const SETTINGS: &str = "settings";
pub const CALENDAR: &str = "calendar";

static SETTINGS_BUILDING: AtomicBool = AtomicBool::new(false);
static CALENDAR_BUILDING: AtomicBool = AtomicBool::new(false);

struct Spec {
    label: &'static str,
    url: &'static str,
    title: &'static str,
    width: f64,
    height: f64,
    min_width: f64,
    min_height: f64,
}

const SETTINGS_SPEC: Spec = Spec {
    label: SETTINGS,
    url: "settings.html",
    title: "Settings",
    width: 760.0,
    height: 620.0,
    min_width: 620.0,
    min_height: 480.0,
};

const CALENDAR_SPEC: Spec = Spec {
    label: CALENDAR,
    url: "calendar.html",
    title: "Calendar",
    width: 880.0,
    height: 600.0,
    min_width: 700.0,
    min_height: 480.0,
};

fn spec_for(label: &str) -> Option<&'static Spec> {
    match label {
        SETTINGS => Some(&SETTINGS_SPEC),
        CALENDAR => Some(&CALENDAR_SPEC),
        _ => None,
    }
}

fn building_flag(label: &str) -> Option<&'static AtomicBool> {
    match label {
        SETTINGS => Some(&SETTINGS_BUILDING),
        CALENDAR => Some(&CALENDAR_BUILDING),
        _ => None,
    }
}

fn build(app: &AppHandle, spec: &Spec) -> Option<WebviewWindow> {
    let window = WebviewWindowBuilder::new(app, spec.label, WebviewUrl::App(spec.url.into()))
        .title(spec.title)
        .inner_size(spec.width, spec.height)
        .min_inner_size(spec.min_width, spec.min_height)
        .decorations(false)
        .transparent(true)
        .resizable(true)
        .maximizable(false)
        .minimizable(true)
        .closable(false)
        .always_on_top(false)
        .visible_on_all_workspaces(false)
        .focused(true)
        .skip_taskbar(false)
        .shadow(true)
        .visible(false)
        .center()
        .build()
        .ok()?;

    #[cfg(target_os = "windows")]
    {
        let _ = window_vibrancy::apply_acrylic(&window, Some((10, 10, 15, 20)));
    }
    if let Ok(hwnd) = window.hwnd() {
        crate::active_look::keep_active_look(hwnd);
    }

    let win = window.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let _ = win.hide();
            set_memory_level(&win, true);
            set_suspended(&win, true);
            crate::mem::trim_soon();
        }
    });

    Some(window)
}

pub fn open(app: &AppHandle, label: &str) {
    crate::mem::mark_interaction();

    if let Some(win) = app.get_webview_window(label) {
        present(&win);
        return;
    }

    let Some(spec) = spec_for(label) else { return };
    let Some(flag) = building_flag(label) else { return };

    if flag.swap(true, Ordering::SeqCst) {
        return;
    }

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Some(win) = build(&app, spec) {
            present(&win);
        }
        flag.store(false, Ordering::SeqCst);
    });
}

fn present(win: &WebviewWindow) {
    set_suspended(win, false);
    set_memory_level(win, false);
    let _ = win.show();
    let _ = win.unminimize();
    let _ = win.set_focus();
    if let Ok(hwnd) = win.hwnd() {
        unsafe {
            use windows::Win32::UI::WindowsAndMessaging::{
                SetForegroundWindow, ShowWindow, SW_RESTORE, SW_SHOW,
            };
            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = ShowWindow(hwnd, SW_RESTORE);
            let _ = SetForegroundWindow(hwnd);
        }
    }
}

fn set_memory_level(win: &WebviewWindow, low: bool) {
    let _ = win.with_webview(move |webview| unsafe {
        use webview2_com::Microsoft::Web::WebView2::Win32::{
            ICoreWebView2_19, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW,
            COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL,
        };
        use windows_core::Interface;

        let Ok(core) = webview.controller().CoreWebView2() else {
            return;
        };
        let Ok(core) = core.cast::<ICoreWebView2_19>() else {
            return;
        };
        let level = if low {
            COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW
        } else {
            COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL
        };
        let _ = core.SetMemoryUsageTargetLevel(level);
    });
}

fn set_suspended(win: &WebviewWindow, suspend: bool) {
    let _ = win.with_webview(move |webview| unsafe {
        use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2_3;
        use windows_core::Interface;

        let Ok(core) = webview.controller().CoreWebView2() else {
            return;
        };
        let Ok(core) = core.cast::<ICoreWebView2_3>() else {
            return;
        };

        if suspend {
            let _ = core.TrySuspend(
                &webview2_com::TrySuspendCompletedHandler::create(Box::new(|_, _| Ok(()))),
            );
        } else {
            let _ = core.Resume();
        }
    });
}
