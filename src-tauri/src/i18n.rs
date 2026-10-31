use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use tauri::menu::MenuItem;
use tauri::{AppHandle, Wry};

// Keep in step with LANGUAGES in src/i18n.ts: one entry per file in src/locales.
const LOCALES: &[(&str, &str)] = &[("en", include_str!("../../src/locales/en.json"))];

type Messages = HashMap<String, String>;

fn catalog() -> &'static HashMap<&'static str, Messages> {
    static CATALOG: OnceLock<HashMap<&'static str, Messages>> = OnceLock::new();
    CATALOG.get_or_init(|| {
        LOCALES
            .iter()
            .filter_map(|(code, raw)| serde_json::from_str(raw).ok().map(|m| (*code, m)))
            .collect()
    })
}

fn system_language() -> String {
    use windows::Win32::Globalization::GetUserDefaultLocaleName;
    let mut buf = [0u16; 85];
    let len = unsafe { GetUserDefaultLocaleName(&mut buf) };
    if len <= 0 {
        return "en".into();
    }
    String::from_utf16_lossy(&buf[..len as usize - 1])
        .split('-')
        .next()
        .unwrap_or("en")
        .to_lowercase()
}

fn language(app: &AppHandle) -> String {
    let preference = crate::utils::get_setting_str(app, "nectar-language").unwrap_or_else(|| "system".into());
    let wanted = if preference == "system" { system_language() } else { preference };
    if catalog().contains_key(wanted.as_str()) { wanted } else { "en".into() }
}

/// Translates a key from src/locales, falling back to English and then to the key itself.
pub fn tr(app: &AppHandle, key: &str) -> String {
    let catalog = catalog();
    catalog
        .get(language(app).as_str())
        .and_then(|m| m.get(key))
        .or_else(|| catalog.get("en").and_then(|m| m.get(key)))
        .cloned()
        .unwrap_or_else(|| key.to_string())
}

pub struct TrayItem {
    pub item: MenuItem<Wry>,
    pub key: &'static str,
}

fn tray_items() -> &'static Mutex<Vec<TrayItem>> {
    static ITEMS: OnceLock<Mutex<Vec<TrayItem>>> = OnceLock::new();
    ITEMS.get_or_init(|| Mutex::new(Vec::new()))
}

pub fn register_tray_item(item: MenuItem<Wry>, key: &'static str) {
    if let Ok(mut items) = tray_items().lock() {
        items.push(TrayItem { item, key });
    }
}

pub fn refresh_tray_labels(app: &AppHandle) {
    if let Ok(items) = tray_items().lock() {
        for TrayItem { item, key } in items.iter() {
            let _ = item.set_text(tr(app, key));
        }
    }
}
