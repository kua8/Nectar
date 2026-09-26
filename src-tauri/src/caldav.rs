use std::time::Duration;

use chrono::{Duration as Days, Local, NaiveDate, NaiveDateTime, TimeZone, Utc};
use reqwest::Url;
use tauri::AppHandle;

use crate::calendar_sync::{
    self, CalEvent, CalInfo, SourceData, ACCOUNT_KEY, LOOKAHEAD_DAYS, LOOKBACK_DAYS,
};

const KEYRING_SERVICE: &str = "Nectar CalDAV";
pub const AUTH_ERROR: &str = "Login failed. Check the username and app password.";
const CALDAV_NS: &str = "urn:ietf:params:xml:ns:caldav";

pub struct Creds {
    pub url: String,
    pub username: String,
    pub password: String,
}

fn entry(username: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYRING_SERVICE, username).map_err(|e| e.to_string())
}

fn load_password(username: &str) -> Option<String> {
    entry(username).ok()?.get_password().ok()
}

fn configured(app: &AppHandle) -> Option<(String, String)> {
    let enabled = crate::utils::get_setting_str(app, "nectar-caldav-enabled").as_deref() == Some("true");
    let url = crate::utils::get_setting_str(app, "nectar-caldav-url").filter(|s| !s.is_empty())?;
    let username = crate::utils::get_setting_str(app, "nectar-caldav-username").filter(|s| !s.is_empty())?;
    enabled.then_some((url, username))
}

pub fn stored_credentials(app: &AppHandle) -> Option<Creds> {
    let (url, username) = configured(app)?;
    let password = load_password(&username)?;
    Some(Creds { url, username, password })
}

pub fn is_connected(app: &AppHandle) -> bool {
    configured(app).map(|(_, user)| load_password(&user).is_some()).unwrap_or(false)
}

pub fn error_chain(e: &dyn std::error::Error) -> String {
    let mut parts = vec![e.to_string()];
    let mut source = e.source();
    while let Some(s) = source {
        parts.push(s.to_string());
        source = s.source();
    }
    parts.join(": ")
}

fn normalize_base(raw: &str) -> Result<Url, String> {
    let mut s = raw.trim().to_string();
    if !s.contains("://") {
        s = format!("https://{}", s);
    }
    let mut url = Url::parse(&s).map_err(|_| "That server address isn't valid.".to_string())?;
    if url.path().is_empty() || url.path() == "/" {
        url.set_path("/remote.php/dav/");
    }
    Ok(url)
}

async fn dav(
    client: &reqwest::Client,
    method: &str,
    url: &Url,
    creds: &Creds,
    depth: &str,
    body: String,
) -> Result<String, String> {
    let method_name = method.to_string();
    let method = reqwest::Method::from_bytes(method.as_bytes()).map_err(|e| e.to_string())?;
    let resp = client
        .request(method, url.clone())
        .basic_auth(&creds.username, Some(&creds.password))
        .header("Depth", depth)
        .header("Content-Type", "application/xml; charset=utf-8")
        .body(body)
        .send()
        .await
        .map_err(|e| {
            let detail = error_chain(&e);
            eprintln!("[caldav] {} {} failed: {}", method_name, url, detail);
            if e.is_timeout() {
                "The server took too long to respond.".to_string()
            } else if e.is_connect() {
                format!("Couldn't connect to {}: {}", url.host_str().unwrap_or("the server"), detail)
            } else {
                format!("Request failed: {}", detail)
            }
        })?;
    let status = resp.status();
    eprintln!("[caldav] {} {} -> {}", method_name, url, status);
    if status == reqwest::StatusCode::UNAUTHORIZED {
        return Err(AUTH_ERROR.to_string());
    }
    if !status.is_success() {
        return Err(format!("The server answered {} at {}.", status.as_u16(), url));
    }
    resp.text().await.map_err(|e| e.to_string())
}

fn href_inside(doc: &roxmltree::Document, parent: &str) -> Option<String> {
    doc.descendants()
        .find(|n| n.is_element() && n.tag_name().name() == parent)
        .and_then(|n| n.descendants().find(|c| c.is_element() && c.tag_name().name() == "href"))
        .and_then(|h| h.text())
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
}

const DISCOVERY_BODY: &str = r#"<?xml version="1.0" encoding="utf-8"?><d:propfind xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav"><d:prop><d:current-user-principal/><c:calendar-home-set/></d:prop></d:propfind>"#;

const LIST_BODY: &str = r#"<?xml version="1.0" encoding="utf-8"?><d:propfind xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav" xmlns:a="http://apple.com/ns/ical/"><d:prop><d:resourcetype/><d:displayname/><a:calendar-color/><c:supported-calendar-component-set/></d:prop></d:propfind>"#;

async fn discover_home(client: &reqwest::Client, creds: &Creds, base: &Url) -> Result<Url, String> {
    let xml = dav(client, "PROPFIND", base, creds, "0", DISCOVERY_BODY.to_string()).await?;
    let (home, principal) = {
        let doc = roxmltree::Document::parse(&xml).map_err(|_| "The server sent an unreadable response.".to_string())?;
        (href_inside(&doc, "calendar-home-set"), href_inside(&doc, "current-user-principal"))
    };
    if let Some(home) = home {
        return base.join(&home).map_err(|e| e.to_string());
    }
    if let Some(principal) = principal {
        let principal_url = base.join(&principal).map_err(|e| e.to_string())?;
        let xml = dav(client, "PROPFIND", &principal_url, creds, "0", DISCOVERY_BODY.to_string()).await?;
        let home = {
            let doc = roxmltree::Document::parse(&xml).map_err(|_| "The server sent an unreadable response.".to_string())?;
            href_inside(&doc, "calendar-home-set")
        };
        if let Some(home) = home {
            return principal_url.join(&home).map_err(|e| e.to_string());
        }
    }
    Ok(base.clone())
}

struct RemoteCalendar {
    url: Url,
    name: String,
    color: Option<String>,
}

async fn list_calendars(client: &reqwest::Client, creds: &Creds, home: &Url) -> Result<Vec<RemoteCalendar>, String> {
    let xml = dav(client, "PROPFIND", home, creds, "1", LIST_BODY.to_string()).await?;
    parse_calendar_list(&xml, home)
}

fn parse_calendar_list(xml: &str, home: &Url) -> Result<Vec<RemoteCalendar>, String> {
    let doc = roxmltree::Document::parse(xml).map_err(|_| "The server sent an unreadable response.".to_string())?;
    let mut out = Vec::new();
    for resp in doc.descendants().filter(|n| n.is_element() && n.tag_name().name() == "response") {
        let is_calendar = resp.descendants().any(|n| {
            n.is_element()
                && n.tag_name().name() == "calendar"
                && n.tag_name().namespace() == Some(CALDAV_NS)
                && n.parent().map(|p| p.tag_name().name() == "resourcetype").unwrap_or(false)
        });
        if !is_calendar {
            continue;
        }
        let comps: Vec<&str> = resp
            .descendants()
            .filter(|n| n.is_element() && n.tag_name().name() == "comp")
            .filter_map(|n| n.attribute("name"))
            .collect();
        if !comps.is_empty() && !comps.iter().any(|c| c.eq_ignore_ascii_case("VEVENT")) {
            continue;
        }
        let Some(href) = resp
            .children()
            .find(|n| n.is_element() && n.tag_name().name() == "href")
            .and_then(|n| n.text())
        else {
            continue;
        };
        let Ok(url) = home.join(href.trim()) else { continue };
        let text_of = |name: &str| {
            resp.descendants()
                .find(|n| n.is_element() && n.tag_name().name() == name)
                .and_then(|n| n.text())
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
        };
        let color = text_of("calendar-color").map(|c| if c.starts_with('#') && c.len() == 9 { c[..7].to_string() } else { c });
        let name = text_of("displayname").unwrap_or_else(|| "Calendar".to_string());
        out.push(RemoteCalendar { url, name, color });
    }
    Ok(out)
}

pub fn unfold(ics: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for raw in ics.split('\n') {
        let line = raw.trim_end_matches('\r');
        if (line.starts_with(' ') || line.starts_with('\t')) && !out.is_empty() {
            if let Some(last) = out.last_mut() {
                last.push_str(&line[1..]);
            }
        } else {
            out.push(line.to_string());
        }
    }
    out
}

pub fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') | Some('N') => out.push('\n'),
                Some(other) => out.push(other),
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    out
}

pub fn parse_when(params: &str, value: &str) -> Option<(i64, bool)> {
    let v = value.trim();
    let params = params.to_ascii_uppercase();
    if params.contains("VALUE=DATE") && !params.contains("DATE-TIME") || v.len() == 8 {
        let date = NaiveDate::parse_from_str(v, "%Y%m%d").ok()?;
        let local = Local.from_local_datetime(&date.and_hms_opt(0, 0, 0)?).earliest()?;
        return Some((local.timestamp_millis(), true));
    }
    if let Some(utc) = v.strip_suffix('Z') {
        let naive = NaiveDateTime::parse_from_str(utc, "%Y%m%dT%H%M%S").ok()?;
        return Some((Utc.from_utc_datetime(&naive).timestamp_millis(), false));
    }
    let naive = NaiveDateTime::parse_from_str(v, "%Y%m%dT%H%M%S").ok()?;
    Some((Local.from_local_datetime(&naive).earliest()?.timestamp_millis(), false))
}

#[derive(Default)]
struct RawEvent {
    uid: String,
    summary: String,
    location: Option<String>,
    status: String,
    start: Option<(String, String)>,
    end: Option<(String, String)>,
}

fn parse_ics(ics: &str, calendar: &str, color: &Option<String>) -> Vec<CalEvent> {
    let mut events = Vec::new();
    let mut current: Option<RawEvent> = None;
    let mut in_alarm = false;

    for line in unfold(ics) {
        let Some((head, value)) = line.split_once(':') else { continue };
        let mut parts = head.split(';');
        let name = parts.next().unwrap_or("").to_ascii_uppercase();
        let params = parts.collect::<Vec<_>>().join(";");

        match (name.as_str(), value.trim()) {
            ("BEGIN", "VEVENT") => current = Some(RawEvent::default()),
            ("BEGIN", "VALARM") => in_alarm = true,
            ("END", "VALARM") => in_alarm = false,
            ("END", "VEVENT") => {
                if let Some(raw) = current.take() {
                    if let Some(event) = finish_event(raw, calendar, color) {
                        events.push(event);
                    }
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
                        "DTSTART" => raw.start = Some((params, value.to_string())),
                        "DTEND" => raw.end = Some((params, value.to_string())),
                        _ => {}
                    }
                }
            }
        }
    }
    events
}

fn finish_event(raw: RawEvent, calendar: &str, color: &Option<String>) -> Option<CalEvent> {
    if raw.status == "CANCELLED" {
        return None;
    }
    let (start_params, start_value) = raw.start?;
    let (start, all_day) = parse_when(&start_params, &start_value)?;
    let end = raw
        .end
        .and_then(|(p, v)| parse_when(&p, &v))
        .map(|(ms, _)| ms)
        .unwrap_or(if all_day { start + 86_400_000 } else { start });
    let title = raw.summary.trim().to_string();
    Some(CalEvent {
        uid: raw.uid,
        calendar: calendar.to_string(),
        color: color.clone(),
        title: if title.is_empty() { "(No title)".to_string() } else { title },
        location: raw.location,
        all_day,
        start,
        end: end.max(start + 1),
    })
}

async fn fetch_events(
    client: &reqwest::Client,
    creds: &Creds,
    cal: &RemoteCalendar,
    from: &str,
    to: &str,
) -> Result<Vec<CalEvent>, String> {
    let body = format!(
        r#"<?xml version="1.0" encoding="utf-8"?><c:calendar-query xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav"><d:prop><d:getetag/><c:calendar-data><c:expand start="{from}" end="{to}"/></c:calendar-data></d:prop><c:filter><c:comp-filter name="VCALENDAR"><c:comp-filter name="VEVENT"><c:time-range start="{from}" end="{to}"/></c:comp-filter></c:comp-filter></c:filter></c:calendar-query>"#
    );
    let xml = dav(client, "REPORT", &cal.url, creds, "1", body).await?;
    parse_report(&xml, &cal.name, &cal.color)
}

fn parse_report(xml: &str, calendar: &str, color: &Option<String>) -> Result<Vec<CalEvent>, String> {
    let doc = roxmltree::Document::parse(xml).map_err(|_| "The server sent an unreadable response.".to_string())?;
    let mut events = Vec::new();
    for node in doc.descendants().filter(|n| n.is_element() && n.tag_name().name() == "calendar-data") {
        if let Some(ics) = node.text() {
            events.extend(parse_ics(ics, calendar, color));
        }
    }
    Ok(events)
}

pub async fn fetch_all(creds: &Creds) -> Result<(Vec<CalInfo>, Vec<CalEvent>), String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("Nectar")
        .build()
        .map_err(|e| e.to_string())?;
    let base = normalize_base(&creds.url)?;
    let home = discover_home(&client, creds, &base).await?;
    let calendars = list_calendars(&client, creds, &home).await?;
    if calendars.is_empty() {
        return Err("No calendars were found at this address.".to_string());
    }

    let now = Local::now();
    let fmt = |t: chrono::DateTime<Local>| t.with_timezone(&Utc).format("%Y%m%dT%H%M%SZ").to_string();
    let from = fmt(now - Days::days(LOOKBACK_DAYS));
    let to = fmt(now + Days::days(LOOKAHEAD_DAYS));

    let mut events = Vec::new();
    let mut succeeded = 0;
    let mut last_error = None;
    for cal in &calendars {
        match fetch_events(&client, creds, cal, &from, &to).await {
            Ok(mut list) => {
                succeeded += 1;
                events.append(&mut list);
            }
            Err(e) if e == AUTH_ERROR => return Err(e),
            Err(e) => last_error = Some(e),
        }
    }
    if succeeded == 0 {
        return Err(last_error.unwrap_or_else(|| "Couldn't read any calendar.".to_string()));
    }
    events.sort_by_key(|e| e.start);
    let infos = calendars.into_iter().map(|c| CalInfo { name: c.name, color: c.color }).collect();
    Ok((infos, events))
}

#[tauri::command]
pub async fn caldav_connect(
    app: AppHandle,
    url: String,
    username: String,
    password: String,
) -> Result<calendar_sync::CalendarState, String> {
    let url = url.trim().to_string();
    let username = username.trim().to_string();
    if url.is_empty() || username.is_empty() || password.is_empty() {
        return Err("Enter the server address, username and app password.".to_string());
    }

    let (calendars, events) = fetch_all(&Creds {
        url: url.clone(),
        username: username.clone(),
        password: password.clone(),
    })
    .await?;

    if let Some(previous) = crate::utils::get_setting_str(&app, "nectar-caldav-username") {
        if previous != username {
            if let Ok(old) = entry(&previous) {
                let _ = old.delete_credential();
            }
        }
    }
    entry(&username)?
        .set_password(&password)
        .map_err(|e| format!("Couldn't save the password to Windows Credential Manager: {}", e))?;

    for (key, value) in [
        ("nectar-caldav-url", url),
        ("nectar-caldav-username", username),
        ("nectar-caldav-enabled", "true".to_string()),
    ] {
        crate::commands::save_setting(app.clone(), key.to_string(), serde_json::Value::String(value))?;
    }

    calendar_sync::put_source(
        &app,
        ACCOUNT_KEY,
        SourceData {
            synced_at: Some(calendar_sync::now_ms()),
            error: None,
            calendars,
            events,
        },
    );
    Ok(calendar_sync::emit_state(&app))
}

#[tauri::command]
pub fn caldav_disconnect(app: AppHandle) -> Result<calendar_sync::CalendarState, String> {
    if let Some(username) = crate::utils::get_setting_str(&app, "nectar-caldav-username") {
        if let Ok(e) = entry(&username) {
            let _ = e.delete_credential();
        }
    }
    calendar_sync::drop_source(&app, ACCOUNT_KEY);
    crate::commands::save_setting(
        app.clone(),
        "nectar-caldav-enabled".to_string(),
        serde_json::Value::String("false".to_string()),
    )?;
    Ok(calendar_sync::emit_state(&app))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ICS: &str = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:a1\r\nSUMMARY:Team sync\\, weekly\r\nLOCATION:Room\r\n 4\r\nDTSTART:20260925T140000Z\r\nDTEND:20260925T150000Z\r\nBEGIN:VALARM\r\nSUMMARY:alarm text\r\nEND:VALARM\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:a2\r\nSUMMARY:Holiday\r\nDTSTART;VALUE=DATE:20260926\r\nDTEND;VALUE=DATE:20260928\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:a3\r\nSUMMARY:Cancelled\r\nSTATUS:CANCELLED\r\nDTSTART:20260925T160000Z\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";

    #[test]
    fn parses_timed_allday_and_skips_cancelled() {
        let events = parse_ics(ICS, "Work", &Some("#0082c9".to_string()));
        assert_eq!(events.len(), 2);

        let timed = &events[0];
        assert_eq!(timed.title, "Team sync, weekly");
        assert_eq!(timed.location.as_deref(), Some("Room4"));
        assert!(!timed.all_day);
        assert_eq!(timed.end - timed.start, 3_600_000);
        assert_eq!(timed.calendar, "Work");

        let allday = &events[1];
        assert!(allday.all_day);
        assert!(allday.end - allday.start >= 2 * 82_800_000);
    }

    #[test]
    fn normalizes_server_addresses() {
        assert_eq!(normalize_base("cloud.example.com").unwrap().as_str(), "https://cloud.example.com/remote.php/dav/");
        assert_eq!(
            normalize_base(" https://cloud.example.com/remote.php/dav ").unwrap().as_str(),
            "https://cloud.example.com/remote.php/dav"
        );
    }

    const LIST_XML: &str = r##"<?xml version="1.0"?>
<d:multistatus xmlns:d="DAV:" xmlns:s="http://sabredav.org/ns" xmlns:cal="urn:ietf:params:xml:ns:caldav" xmlns:cs="http://calendarserver.org/ns/" xmlns:oc="http://owncloud.org/ns" xmlns:nc="http://nextcloud.org/ns">
 <d:response>
  <d:href>/remote.php/dav/calendars/alice/</d:href>
  <d:propstat><d:prop><d:resourcetype><d:collection/></d:resourcetype></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat>
 </d:response>
 <d:response>
  <d:href>/remote.php/dav/calendars/alice/personal/</d:href>
  <d:propstat><d:prop>
   <d:resourcetype><d:collection/><cal:calendar/></d:resourcetype>
   <d:displayname>Personal</d:displayname>
   <x1:calendar-color xmlns:x1="http://apple.com/ns/ical/">#0082c9FF</x1:calendar-color>
   <cal:supported-calendar-component-set><cal:comp name="VEVENT"/><cal:comp name="VTODO"/></cal:supported-calendar-component-set>
  </d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat>
 </d:response>
 <d:response>
  <d:href>/remote.php/dav/calendars/alice/tasks/</d:href>
  <d:propstat><d:prop>
   <d:resourcetype><d:collection/><cal:calendar/></d:resourcetype>
   <d:displayname>Tasks</d:displayname>
   <cal:supported-calendar-component-set><cal:comp name="VTODO"/></cal:supported-calendar-component-set>
  </d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat>
 </d:response>
 <d:response>
  <d:href>/remote.php/dav/calendars/alice/inbox/</d:href>
  <d:propstat><d:prop><d:resourcetype><d:collection/><cal:schedule-inbox/></d:resourcetype></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat>
 </d:response>
 <d:response>
  <d:href>/remote.php/dav/calendars/alice/contact_birthdays/</d:href>
  <d:propstat><d:prop>
   <d:resourcetype><d:collection/><cal:calendar/></d:resourcetype>
   <d:displayname>Contact birthdays</d:displayname>
  </d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat>
 </d:response>
</d:multistatus>"##;

    #[test]
    fn lists_only_event_calendars() {
        let home = Url::parse("https://cloud.example.com/remote.php/dav/calendars/alice/").unwrap();
        let cals = parse_calendar_list(LIST_XML, &home).unwrap();
        let names: Vec<&str> = cals.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["Personal", "Contact birthdays"]);
        assert_eq!(cals[0].color.as_deref(), Some("#0082c9"));
        assert_eq!(cals[0].url.as_str(), "https://cloud.example.com/remote.php/dav/calendars/alice/personal/");
        assert_eq!(cals[1].color, None);
    }

    #[test]
    fn reads_events_from_a_report_with_escaped_carriage_returns() {
        let xml = r##"<?xml version="1.0"?>
<d:multistatus xmlns:d="DAV:" xmlns:cal="urn:ietf:params:xml:ns:caldav">
 <d:response>
  <d:href>/remote.php/dav/calendars/alice/personal/a.ics</d:href>
  <d:propstat><d:prop>
   <d:getetag>"abc"</d:getetag>
   <cal:calendar-data>BEGIN:VCALENDAR&#13;
VERSION:2.0&#13;
BEGIN:VEVENT&#13;
UID:weekly-1&#13;
SUMMARY:Standup&#13;
DTSTART:20260928T083000Z&#13;
DTEND:20260928T084500Z&#13;
RECURRENCE-ID:20260928T083000Z&#13;
END:VEVENT&#13;
END:VCALENDAR&#13;
</cal:calendar-data>
  </d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat>
 </d:response>
</d:multistatus>"##;
        let events = parse_report(xml, "Personal", &None).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].title, "Standup");
        assert_eq!(events[0].end - events[0].start, 900_000);
    }
}
