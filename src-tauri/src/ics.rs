use std::collections::HashSet;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

use chrono::{Duration as Days, Local, NaiveDate, NaiveDateTime, TimeZone, Utc};
use reqwest::Url;
use rrule::{RRuleSet, Tz as RTz};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::caldav::{error_chain, unescape, unfold};
use crate::calendar_sync::{self, link_key, CalEvent, SourceData, LOOKAHEAD_DAYS, LOOKBACK_DAYS};

const KEYRING_SERVICE: &str = "Nectar Calendar Link";
const LINKS_FILE: &str = "calendar_links.json";
const MAX_BYTES: usize = 15 * 1024 * 1024;
const MAX_LINKS: usize = 12;
const MAX_OCCURRENCES: u16 = 1500;
const DAY_MS: i64 = 86_400_000;
const PALETTE: [&str; 6] = ["#4285f4", "#ea4335", "#34a853", "#fbbc04", "#a142f4", "#ff7a59"];

#[derive(Clone, Serialize, Deserialize)]
pub struct LinkMeta {
    pub id: String,
    pub name: String,
    pub provider: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Detection {
    pub provider: String,
    pub label: String,
    pub problem: Option<String>,
}

fn links_path(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|p| p.join(LINKS_FILE))
}

pub fn list_links(app: &AppHandle) -> Vec<LinkMeta> {
    links_path(app)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn write_links(app: &AppHandle, links: &[LinkMeta]) -> Result<(), String> {
    let path = links_path(app).ok_or_else(|| "No config folder available.".to_string())?;
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let json = serde_json::to_string(links).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}

fn secret(id: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYRING_SERVICE, id).map_err(|e| e.to_string())
}

fn saved_url(id: &str) -> Option<String> {
    secret(id).ok()?.get_password().ok()
}

pub fn color_for(id: &str) -> String {
    let sum: usize = id.bytes().map(|b| b as usize).sum();
    PALETTE[sum % PALETTE.len()].to_string()
}

fn detect(url: &Url) -> (&'static str, &'static str) {
    let host = url.host_str().unwrap_or("").to_ascii_lowercase();
    let path = url.path().to_ascii_lowercase();
    let on = |domain: &str| host == domain || host.ends_with(&format!(".{}", domain));
    if on("google.com") {
        ("google", "Google Calendar")
    } else if on("icloud.com") {
        ("icloud", "iCloud")
    } else if on("outlook.live.com") || on("outlook.office365.com") || on("outlook.office.com") || on("outlook.com") {
        ("outlook", "Outlook")
    } else if on("proton.me") || on("protonmail.com") {
        ("proton", "Proton Calendar")
    } else if on("fastmail.com") || on("fastmail.fm") {
        ("fastmail", "Fastmail")
    } else if path.contains("public-calendars") || path.contains("remote.php") {
        ("nextcloud", "Nextcloud")
    } else {
        ("other", "Calendar link")
    }
}

fn normalize_link(raw: &str) -> Result<Url, String> {
    let trimmed = raw.trim();
    let lower = trimmed.to_ascii_lowercase();
    let mut text = if lower.starts_with("webcals://") {
        format!("https://{}", &trimmed[10..])
    } else if lower.starts_with("webcal://") {
        format!("https://{}", &trimmed[9..])
    } else if !trimmed.contains("://") {
        format!("https://{}", trimmed)
    } else {
        trimmed.to_string()
    };
    text = text.replace(' ', "");
    let mut url = Url::parse(&text).map_err(|_| "That doesn't look like a valid link.".to_string())?;
    if url.scheme() != "https" && url.scheme() != "http" {
        return Err("Only http, https and webcal links are supported.".to_string());
    }
    const SHARE_PAGE: &str = "/apps/calendar/p/";
    if let Some(at) = url.path().find(SHARE_PAGE) {
        let path = url.path().to_string();
        let token = path[at + SHARE_PAGE.len()..].split('/').next().unwrap_or("");
        if !token.is_empty() {
            let base = &path[..at];
            let base = base.strip_suffix("/index.php").unwrap_or(base);
            url.set_path(&format!("{}/remote.php/dav/public-calendars/{}", base, token));
            url.set_query(Some("export"));
        }
    }
    if url.path().contains("public-calendars") && url.query().is_none() {
        url.set_query(Some("export"));
    }
    Ok(url)
}

fn problem_with(url: &Url) -> Option<String> {
    let host = url.host_str().unwrap_or("");
    let path = url.path();
    if host.ends_with("google.com") && (path.starts_with("/calendar/embed") || path.starts_with("/calendar/u/") || path.starts_with("/calendar/r")) {
        return Some("That's the Google Calendar web page. Use the \"Secret address in iCal format\" from the calendar's settings instead.".to_string());
    }
    if host.ends_with("outlook.live.com") && path.starts_with("/calendar") {
        return Some("That's the Outlook web page. Publish the calendar and copy its ICS link instead.".to_string());
    }
    if host.ends_with("icloud.com") && path.starts_with("/calendar") {
        return Some("That's the iCloud web page. Share the calendar as a Public Calendar and copy that link instead.".to_string());
    }
    None
}

#[tauri::command]
pub fn ics_detect(url: String) -> Option<Detection> {
    let parsed = normalize_link(&url).ok()?;
    let (provider, label) = detect(&parsed);
    Some(Detection {
        provider: provider.to_string(),
        label: label.to_string(),
        problem: problem_with(&parsed),
    })
}

async fn download(url: &Url) -> Result<String, String> {
    let host = url.host_str().unwrap_or("the server").to_string();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("Nectar")
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .get(url.clone())
        .header("Accept", "text/calendar, */*")
        .send()
        .await
        .map_err(|e| {
            let detail = error_chain(&e);
            eprintln!("[ics] request to {} failed: {}", host, detail);
            if e.is_timeout() {
                "The server took too long to respond.".to_string()
            } else {
                format!("Couldn't reach {}: {}", host, detail)
            }
        })?;
    let status = resp.status();
    eprintln!("[ics] {} -> {}", host, status);
    match status.as_u16() {
        200..=299 => {}
        401 | 403 => return Err("The link was rejected. It may have been turned off or regenerated.".to_string()),
        404 | 410 => return Err("That link doesn't exist anymore. Create a new one and add it again.".to_string()),
        code => return Err(format!("The server answered {}.", code)),
    }
    if resp.content_length().map(|n| n as usize > MAX_BYTES).unwrap_or(false) {
        return Err("That calendar file is too large.".to_string());
    }
    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
    if bytes.len() > MAX_BYTES {
        return Err("That calendar file is too large.".to_string());
    }
    let text = String::from_utf8_lossy(&bytes).into_owned();
    if !text.contains("BEGIN:VCALENDAR") {
        return Err("That link didn't return a calendar file. Use the .ics / iCal link, not a web page.".to_string());
    }
    Ok(text)
}

#[derive(Default, Clone)]
struct Prop {
    params: String,
    value: String,
}

#[derive(Default)]
struct Raw {
    uid: String,
    summary: String,
    location: Option<String>,
    status: String,
    start: Option<Prop>,
    end: Option<Prop>,
    duration: Option<String>,
    rrules: Vec<String>,
    exdates: Vec<Prop>,
    recurrence_id: Option<Prop>,
}

fn parse_components(ics: &str) -> Vec<Raw> {
    let mut out = Vec::new();
    let mut current: Option<Raw> = None;
    let mut in_alarm = false;

    for line in unfold(ics) {
        let Some((head, value)) = line.split_once(':') else { continue };
        let mut parts = head.split(';');
        let name = parts.next().unwrap_or("").to_ascii_uppercase();
        let params = parts.collect::<Vec<_>>().join(";");
        let prop = || Prop { params: params.clone(), value: value.trim().to_string() };

        match (name.as_str(), value.trim()) {
            ("BEGIN", "VEVENT") => current = Some(Raw::default()),
            ("BEGIN", "VALARM") => in_alarm = true,
            ("END", "VALARM") => in_alarm = false,
            ("END", "VEVENT") => {
                if let Some(raw) = current.take() {
                    out.push(raw);
                }
            }
            _ if in_alarm => {}
            _ => {
                if let Some(raw) = current.as_mut() {
                    match name.as_str() {
                        "UID" => raw.uid = value.trim().to_string(),
                        "SUMMARY" => raw.summary = unescape(value),
                        "LOCATION" => raw.location = Some(unescape(value)).filter(|l| !l.trim().is_empty()),
                        "STATUS" => raw.status = value.trim().to_ascii_uppercase(),
                        "DTSTART" => raw.start = Some(prop()),
                        "DTEND" => raw.end = Some(prop()),
                        "DURATION" => raw.duration = Some(value.trim().to_string()),
                        "RRULE" => raw.rrules.push(value.trim().to_string()),
                        "EXDATE" => raw.exdates.push(prop()),
                        "RECURRENCE-ID" => raw.recurrence_id = Some(prop()),
                        _ => {}
                    }
                }
            }
        }
    }
    out
}

fn windows_zone(name: &str) -> Option<&'static str> {
    const ZONES: &[(&str, &str)] = &[
        ("UTC", "UTC"),
        ("GMT Standard Time", "Europe/London"),
        ("Greenwich Standard Time", "Atlantic/Reykjavik"),
        ("W. Europe Standard Time", "Europe/Berlin"),
        ("Central Europe Standard Time", "Europe/Budapest"),
        ("Romance Standard Time", "Europe/Paris"),
        ("Central European Standard Time", "Europe/Warsaw"),
        ("E. Europe Standard Time", "Europe/Chisinau"),
        ("FLE Standard Time", "Europe/Kiev"),
        ("GTB Standard Time", "Europe/Bucharest"),
        ("Russian Standard Time", "Europe/Moscow"),
        ("Turkey Standard Time", "Europe/Istanbul"),
        ("Israel Standard Time", "Asia/Jerusalem"),
        ("Egypt Standard Time", "Africa/Cairo"),
        ("South Africa Standard Time", "Africa/Johannesburg"),
        ("E. Africa Standard Time", "Africa/Nairobi"),
        ("W. Central Africa Standard Time", "Africa/Lagos"),
        ("Morocco Standard Time", "Africa/Casablanca"),
        ("Arab Standard Time", "Asia/Riyadh"),
        ("Arabian Standard Time", "Asia/Dubai"),
        ("Iran Standard Time", "Asia/Tehran"),
        ("Jordan Standard Time", "Asia/Amman"),
        ("Middle East Standard Time", "Asia/Beirut"),
        ("West Asia Standard Time", "Asia/Tashkent"),
        ("Pakistan Standard Time", "Asia/Karachi"),
        ("India Standard Time", "Asia/Kolkata"),
        ("Bangladesh Standard Time", "Asia/Dhaka"),
        ("SE Asia Standard Time", "Asia/Bangkok"),
        ("China Standard Time", "Asia/Shanghai"),
        ("Singapore Standard Time", "Asia/Singapore"),
        ("Taipei Standard Time", "Asia/Taipei"),
        ("Tokyo Standard Time", "Asia/Tokyo"),
        ("Korea Standard Time", "Asia/Seoul"),
        ("AUS Eastern Standard Time", "Australia/Sydney"),
        ("E. Australia Standard Time", "Australia/Brisbane"),
        ("Cen. Australia Standard Time", "Australia/Adelaide"),
        ("W. Australia Standard Time", "Australia/Perth"),
        ("New Zealand Standard Time", "Pacific/Auckland"),
        ("Hawaiian Standard Time", "Pacific/Honolulu"),
        ("Alaskan Standard Time", "America/Anchorage"),
        ("Pacific Standard Time", "America/Los_Angeles"),
        ("Mountain Standard Time", "America/Denver"),
        ("US Mountain Standard Time", "America/Phoenix"),
        ("Central Standard Time", "America/Chicago"),
        ("Canada Central Standard Time", "America/Regina"),
        ("Central America Standard Time", "America/Guatemala"),
        ("Mexico Standard Time", "America/Mexico_City"),
        ("Eastern Standard Time", "America/New_York"),
        ("SA Pacific Standard Time", "America/Bogota"),
        ("Atlantic Standard Time", "America/Halifax"),
        ("Newfoundland Standard Time", "America/St_Johns"),
        ("E. South America Standard Time", "America/Sao_Paulo"),
        ("Argentina Standard Time", "America/Argentina/Buenos_Aires"),
    ];
    ZONES.iter().find(|(win, _)| win.eq_ignore_ascii_case(name)).map(|(_, iana)| *iana)
}

fn resolve_tzid(tzid: &str) -> Option<chrono_tz::Tz> {
    let name = tzid.trim().trim_matches('"');
    if let Ok(zone) = chrono_tz::Tz::from_str(name) {
        return Some(zone);
    }
    let parts: Vec<&str> = name.split('/').collect();
    for take in [2usize, 1] {
        if parts.len() >= take {
            if let Ok(zone) = chrono_tz::Tz::from_str(&parts[parts.len() - take..].join("/")) {
                return Some(zone);
            }
        }
    }
    windows_zone(name).and_then(|iana| chrono_tz::Tz::from_str(iana).ok())
}

fn param(params: &str, key: &str) -> Option<String> {
    params.split(';').find_map(|p| {
        let (k, v) = p.split_once('=')?;
        k.trim().eq_ignore_ascii_case(key).then(|| v.trim().trim_matches('"').to_string())
    })
}

fn is_date_only(prop: &Prop) -> bool {
    let params = prop.params.to_ascii_uppercase();
    (params.contains("VALUE=DATE") && !params.contains("DATE-TIME")) || prop.value.trim().len() == 8
}

fn when(prop: &Prop) -> Option<(i64, bool)> {
    let value = prop.value.trim();
    if is_date_only(prop) {
        let date = NaiveDate::parse_from_str(value, "%Y%m%d").ok()?;
        let local = Local.from_local_datetime(&date.and_hms_opt(0, 0, 0)?).earliest()?;
        return Some((local.timestamp_millis(), true));
    }
    if let Some(utc) = value.strip_suffix('Z') {
        let naive = NaiveDateTime::parse_from_str(utc, "%Y%m%dT%H%M%S").ok()?;
        return Some((Utc.from_utc_datetime(&naive).timestamp_millis(), false));
    }
    let naive = NaiveDateTime::parse_from_str(value, "%Y%m%dT%H%M%S").ok()?;
    let ms = match param(&prop.params, "TZID").and_then(|t| resolve_tzid(&t)) {
        Some(zone) => zone.from_local_datetime(&naive).earliest()?.timestamp_millis(),
        None => Local.from_local_datetime(&naive).earliest()?.timestamp_millis(),
    };
    Some((ms, false))
}

fn parse_duration_ms(value: &str) -> Option<i64> {
    let body = value.trim().trim_start_matches('+').strip_prefix('P')?;
    let mut total = 0i64;
    let mut digits = String::new();
    let mut in_time = false;
    for c in body.chars() {
        match c {
            'T' => in_time = true,
            '0'..='9' => digits.push(c),
            'W' | 'D' | 'H' | 'M' | 'S' => {
                let n: i64 = digits.parse().ok()?;
                digits.clear();
                total += n * match (c, in_time) {
                    ('W', _) => 7 * DAY_MS,
                    ('D', _) => DAY_MS,
                    ('H', _) => 3_600_000,
                    ('M', true) => 60_000,
                    ('S', _) => 1000,
                    _ => return None,
                };
            }
            _ => return None,
        }
    }
    Some(total)
}

fn duration_of(raw: &Raw, start: i64, all_day: bool) -> i64 {
    if let Some((end, _)) = raw.end.as_ref().and_then(when) {
        (end - start).max(0)
    } else if let Some(d) = raw.duration.as_deref().and_then(parse_duration_ms) {
        d
    } else if all_day {
        DAY_MS
    } else {
        0
    }
}

fn date_line(name: &str, prop: &Prop, all_day: bool) -> String {
    let value = prop.value.trim();
    if all_day {
        let floating: Vec<String> = value
            .split(',')
            .map(|v| if v.len() == 8 { format!("{}T000000", v) } else { v.trim_end_matches('Z').to_string() })
            .collect();
        return format!("{}:{}", name, floating.join(","));
    }
    if value.ends_with('Z') {
        return format!("{}:{}", name, value);
    }
    match param(&prop.params, "TZID").and_then(|t| resolve_tzid(&t)) {
        Some(zone) => format!("{};TZID={}:{}", name, zone.name(), value),
        None => format!("{}:{}", name, value),
    }
}

fn normalize_rrule(rule: &str, all_day: bool) -> String {
    rule.split(';')
        .map(|part| {
            let upper = part.to_ascii_uppercase();
            match upper.strip_prefix("UNTIL=") {
                Some(until) if all_day => {
                    let base = until.trim_end_matches('Z');
                    if base.len() == 8 { format!("UNTIL={}T235959", base) } else { format!("UNTIL={}", base) }
                }
                Some(until) if until.len() == 8 => format!("UNTIL={}T235959Z", until),
                Some(until) if !until.ends_with('Z') => format!("UNTIL={}Z", until),
                _ => upper,
            }
        })
        .collect::<Vec<_>>()
        .join(";")
}

fn occurrences(raw: &Raw, start: &Prop, start_ms: i64, all_day: bool, duration: i64, from: i64, to: i64) -> Vec<i64> {
    let single = || vec![start_ms];
    if raw.rrules.is_empty() {
        return single();
    }
    let mut lines = vec![date_line("DTSTART", start, all_day)];
    lines.extend(raw.rrules.iter().map(|r| format!("RRULE:{}", normalize_rrule(r, all_day))));
    lines.extend(raw.exdates.iter().map(|p| date_line("EXDATE", p, all_day)));

    let set = match RRuleSet::from_str(&lines.join("\n")) {
        Ok(set) => set,
        Err(e) => {
            eprintln!("[ics] couldn't read a repeat rule ({}), showing the first occurrence only", e);
            return single();
        }
    };
    let bound = |ms: i64| Utc.timestamp_millis_opt(ms).single().map(|d| d.with_timezone(&RTz::UTC));
    let (Some(after), Some(before)) = (bound(from - duration), bound(to)) else { return single() };
    set.after(after)
        .before(before)
        .all(MAX_OCCURRENCES)
        .dates
        .into_iter()
        .map(|d| d.timestamp_millis())
        .collect()
}

pub fn events_from_ics(ics: &str, calendar: &str, color: &Option<String>, from: i64, to: i64) -> Vec<CalEvent> {
    let components = parse_components(ics);

    let mut replaced: HashSet<(String, i64)> = HashSet::new();
    for c in &components {
        if let Some((ms, _)) = c.recurrence_id.as_ref().and_then(when) {
            replaced.insert((c.uid.clone(), ms));
        }
    }

    let mut events = Vec::new();
    for raw in &components {
        if raw.status == "CANCELLED" {
            continue;
        }
        let Some(start_prop) = raw.start.as_ref() else { continue };
        let Some((start_ms, all_day)) = when(start_prop) else { continue };
        let duration = duration_of(raw, start_ms, all_day);

        let starts = if raw.recurrence_id.is_some() {
            vec![start_ms]
        } else {
            occurrences(raw, start_prop, start_ms, all_day, duration, from, to)
                .into_iter()
                .filter(|ms| !replaced.contains(&(raw.uid.clone(), *ms)))
                .collect()
        };

        let title = raw.summary.trim();
        for start in starts {
            let end = (start + duration).max(start + 1);
            if start < to && end > from {
                events.push(CalEvent {
                    uid: raw.uid.clone(),
                    calendar: calendar.to_string(),
                    color: color.clone(),
                    title: if title.is_empty() { "(No title)".to_string() } else { title.to_string() },
                    location: raw.location.clone(),
                    all_day,
                    start,
                    end,
                });
            }
        }
    }
    events.sort_by_key(|e| e.start);
    events
}

fn window() -> (i64, i64) {
    let now = Local::now();
    (
        (now - Days::days(LOOKBACK_DAYS)).timestamp_millis(),
        (now + Days::days(LOOKAHEAD_DAYS)).timestamp_millis(),
    )
}

async fn load_events(url: &Url, name: String, color: String) -> Result<Vec<CalEvent>, String> {
    let text = download(url).await?;
    let (from, to) = window();
    tauri::async_runtime::spawn_blocking(move || events_from_ics(&text, &name, &Some(color), from, to))
        .await
        .map_err(|e| e.to_string())
}

pub async fn fetch_link(_app: &AppHandle, meta: &LinkMeta) -> Result<Vec<CalEvent>, String> {
    let saved = saved_url(&meta.id).ok_or_else(|| "The saved link is missing. Remove it and add it again.".to_string())?;
    let url = normalize_link(&saved)?;
    load_events(&url, meta.name.clone(), color_for(&meta.id)).await
}

#[tauri::command]
pub async fn ics_add_link(app: AppHandle, url: String, name: String) -> Result<calendar_sync::CalendarState, String> {
    let mut links = list_links(&app);
    if links.len() >= MAX_LINKS {
        return Err(format!("You can add up to {} calendar links.", MAX_LINKS));
    }
    let parsed = normalize_link(&url)?;
    if let Some(problem) = problem_with(&parsed) {
        return Err(problem);
    }
    if links.iter().any(|l| saved_url(&l.id).as_deref() == Some(parsed.as_str())) {
        return Err("That calendar link is already added.".to_string());
    }

    let (provider, label) = detect(&parsed);
    let id = format!("{:x}", calendar_sync::now_ms());
    let name = if name.trim().is_empty() { label.to_string() } else { name.trim().to_string() };

    let events = load_events(&parsed, name.clone(), color_for(&id)).await?;

    secret(&id)?
        .set_password(parsed.as_str())
        .map_err(|e| format!("Couldn't save the link to Windows Credential Manager: {}", e))?;
    links.push(LinkMeta { id: id.clone(), name, provider: provider.to_string() });
    write_links(&app, &links)?;

    calendar_sync::put_source(
        &app,
        &link_key(&id),
        SourceData { synced_at: Some(calendar_sync::now_ms()), error: None, calendars: Vec::new(), events },
    );
    Ok(calendar_sync::emit_state(&app))
}

#[tauri::command]
pub fn ics_remove_link(app: AppHandle, id: String) -> Result<calendar_sync::CalendarState, String> {
    if let Ok(entry) = secret(&id) {
        let _ = entry.delete_credential();
    }
    let links: Vec<LinkMeta> = list_links(&app).into_iter().filter(|l| l.id != id).collect();
    write_links(&app, &links)?;
    calendar_sync::drop_source(&app, &link_key(&id));
    Ok(calendar_sync::emit_state(&app))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(y: i32, m: u32, d: u32, h: u32, min: u32) -> i64 {
        Utc.with_ymd_and_hms(y, m, d, h, min, 0).unwrap().timestamp_millis()
    }

    fn starts(events: &[CalEvent]) -> Vec<i64> {
        events.iter().map(|e| e.start).collect()
    }

    #[test]
    fn expands_weekly_with_exdate_and_moved_instance() {
        let ics = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:w1\r\nSUMMARY:Standup\r\nDTSTART;TZID=Europe/Berlin:20260907T090000\r\nDTEND;TZID=Europe/Berlin:20260907T093000\r\nRRULE:FREQ=WEEKLY;BYDAY=MO\r\nEXDATE;TZID=Europe/Berlin:20260914T090000\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:w1\r\nSUMMARY:Standup (moved)\r\nRECURRENCE-ID;TZID=Europe/Berlin:20260921T090000\r\nDTSTART;TZID=Europe/Berlin:20260922T100000\r\nDTEND;TZID=Europe/Berlin:20260922T103000\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let events = events_from_ics(ics, "Work", &None, ms(2026, 9, 1, 0, 0), ms(2026, 10, 5, 0, 0));
        assert_eq!(
            starts(&events),
            vec![
                ms(2026, 9, 7, 7, 0),
                ms(2026, 9, 22, 8, 0),
                ms(2026, 9, 28, 7, 0)
            ]
        );
        assert_eq!(events[1].title, "Standup (moved)");
        assert_eq!(events[0].end - events[0].start, 30 * 60_000);
    }

    #[test]
    fn expands_yearly_all_day_birthdays() {
        let ics = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:b1\r\nSUMMARY:Alex's birthday\r\nDTSTART;VALUE=DATE:19900928\r\nRRULE:FREQ=YEARLY\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let events = events_from_ics(ics, "Birthdays", &None, ms(2026, 9, 1, 0, 0), ms(2026, 10, 30, 0, 0));
        assert_eq!(events.len(), 1);
        assert!(events[0].all_day);
        let local = Local.timestamp_millis_opt(events[0].start).unwrap();
        assert_eq!(local.format("%Y-%m-%d").to_string(), "2026-09-28");
    }

    #[test]
    fn honours_count_and_until() {
        let ics = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:c1\r\nSUMMARY:Course\r\nDTSTART:20260901T100000Z\r\nDTEND:20260901T110000Z\r\nRRULE:FREQ=DAILY;COUNT=3\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:c2\r\nSUMMARY:Sprint\r\nDTSTART:20260901T120000Z\r\nDTEND:20260901T130000Z\r\nRRULE:FREQ=DAILY;UNTIL=20260902T235959Z\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let events = events_from_ics(ics, "X", &None, ms(2026, 8, 1, 0, 0), ms(2026, 12, 1, 0, 0));
        assert_eq!(events.iter().filter(|e| e.title == "Course").count(), 3);
        assert_eq!(events.iter().filter(|e| e.title == "Sprint").count(), 2);
    }

    #[test]
    fn understands_windows_and_prefixed_timezone_names() {
        let outlook = Prop { params: "TZID=W. Europe Standard Time".into(), value: "20260907T090000".into() };
        let thunderbird = Prop { params: "TZID=/mozilla.org/20050126_1/Europe/Berlin".into(), value: "20260907T090000".into() };
        let expected = ms(2026, 9, 7, 7, 0);
        assert_eq!(when(&outlook).unwrap().0, expected);
        assert_eq!(when(&thunderbird).unwrap().0, expected);
    }

    #[test]
    fn parses_durations() {
        assert_eq!(parse_duration_ms("PT1H30M"), Some(5_400_000));
        assert_eq!(parse_duration_ms("P1D"), Some(DAY_MS));
        assert_eq!(parse_duration_ms("P2W"), Some(14 * DAY_MS));
        assert_eq!(parse_duration_ms("nonsense"), None);
    }

    #[test]
    fn normalizes_and_recognises_links() {
        let webcal = normalize_link("webcal://p12-caldav.icloud.com/published/2/abc").unwrap();
        assert_eq!(webcal.as_str(), "https://p12-caldav.icloud.com/published/2/abc");
        assert_eq!(detect(&webcal).0, "icloud");

        let nextcloud = normalize_link("https://cloud.example.com/remote.php/dav/public-calendars/TOKEN").unwrap();
        assert!(nextcloud.as_str().ends_with("?export"));
        assert_eq!(detect(&nextcloud).0, "nextcloud");

        let share_page = normalize_link("https://cloud.example.com/index.php/apps/calendar/p/GfuImnB2zQskrnHA/Personal").unwrap();
        assert_eq!(share_page.as_str(), "https://cloud.example.com/remote.php/dav/public-calendars/GfuImnB2zQskrnHA?export");
        let subdir = normalize_link("https://example.com/nextcloud/index.php/apps/calendar/p/TOKEN").unwrap();
        assert_eq!(subdir.as_str(), "https://example.com/nextcloud/remote.php/dav/public-calendars/TOKEN?export");
        assert_eq!(detect(&share_page).0, "nextcloud");

        let google = normalize_link("https://calendar.google.com/calendar/ical/me%40gmail.com/private-abc/basic.ics").unwrap();
        assert_eq!(detect(&google).0, "google");
        assert!(problem_with(&google).is_none());

        let embed = normalize_link("https://calendar.google.com/calendar/embed?src=me%40gmail.com").unwrap();
        assert!(problem_with(&embed).is_some());

        let proton = normalize_link("https://calendar.proton.me/api/calendar/v1/url/x/calendar.ics?CacheKey=a&PassphraseKey=b").unwrap();
        assert_eq!(detect(&proton).0, "proton");
        let outlook = normalize_link("https://outlook.live.com/owa/calendar/x/y/cid-1/calendar.ics").unwrap();
        assert_eq!(detect(&outlook).0, "outlook");
        assert!(normalize_link("ftp://example.com/a.ics").is_err());
    }

    #[test]
    #[ignore]
    fn real_public_feeds() {
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let feeds = [
            "https://calendar.google.com/calendar/ical/en.usa%23holiday%40group.v.calendar.google.com/public/basic.ics",
            "https://calendar.google.com/calendar/ical/en.uk%23holiday%40group.v.calendar.google.com/public/basic.ics",
            "https://www.thunderbird.net/media/caldata/autogen/GermanHolidays.ics",
            "webcal://www.officeholidays.com/ics/usa",
        ];
        for feed in feeds {
            let url = normalize_link(feed).unwrap();
            let text = runtime.block_on(download(&url)).unwrap();
            let (from, to) = (ms(2026, 1, 1, 0, 0), ms(2027, 1, 1, 0, 0));
            let events = events_from_ics(&text, "t", &None, from, to);
            println!("{} -> {} events, first: {:?}", feed, events.len(), events.first().map(|e| &e.title));
            assert!(!events.is_empty());
        }
    }
}
