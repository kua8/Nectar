use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{mpsc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

const MAX_HEAD: usize = 8192;
const MAX_BODY: usize = 4096;
const MAX_CONNECTIONS: usize = 8;
const ASK_WAIT: u64 = 120;
const SETTING: &str = "nectar-agent-api";

static ACTIVE: AtomicUsize = AtomicUsize::new(0);
static NEXT_ASK: AtomicU64 = AtomicU64::new(1);
static PENDING: Mutex<Option<HashMap<u64, mpsc::Sender<bool>>>> = Mutex::new(None);

#[derive(Deserialize)]
struct Incoming {
    state: String,
    name: Option<String>,
    project: Option<String>,
    window: Option<u64>,
    detail: Option<String>,
}

#[derive(Deserialize)]
struct IncomingAsk {
    name: Option<String>,
    project: Option<String>,
    window: Option<u64>,
    tool: Option<String>,
    detail: Option<String>,
}

#[derive(Serialize, Clone)]
struct Ask {
    id: u64,
    tool: Option<String>,
    detail: Option<String>,
}

#[derive(Serialize, Clone)]
struct Activity {
    state: &'static str,
    name: Option<String>,
    project: Option<String>,
    window: Option<u64>,
    detail: Option<String>,
    ask: Option<Ask>,
    cancelled: Option<u64>,
}

pub fn data_dir() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|dir| PathBuf::from(dir).join("com.kua8.nectar"))
}

fn config_path() -> Option<PathBuf> {
    data_dir().map(|dir| dir.join("agent.json"))
}

fn state_of(raw: &str) -> Option<Option<&'static str>> {
    match raw.trim().to_ascii_lowercase().replace('-', "_").as_str() {
        "working" | "thinking" | "running" => Some(Some("working")),
        "tool" | "tool_running" => Some(Some("tool")),
        "needs_input" | "input" | "waiting" => Some(Some("needs_input")),
        "done" | "completed" | "complete" | "finished" => Some(Some("done")),
        "error" | "failed" => Some(Some("error")),
        "clear" | "idle" | "stop" => Some(None),
        _ => None,
    }
}

fn clean(text: Option<String>, limit: usize) -> Option<String> {
    let cleaned: String = text?.chars().filter(|c| !c.is_control()).take(limit).collect();
    let cleaned = cleaned.trim().to_string();
    (!cleaned.is_empty()).then_some(cleaned)
}

fn random_token() -> Option<String> {
    use windows::Win32::Security::Cryptography::{BCryptGenRandom, BCRYPT_USE_SYSTEM_PREFERRED_RNG};
    let mut bytes = [0u8; 24];
    unsafe { BCryptGenRandom(None, &mut bytes, BCRYPT_USE_SYSTEM_PREFERRED_RNG) }.ok().ok()?;
    Some(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

fn same(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn reply(stream: &mut TcpStream, status: &str) {
    let _ = write!(stream, "HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
}

fn reply_json(stream: &mut TcpStream, body: &str) {
    let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

pub fn answer(id: u64, allow: bool) {
    let sender = PENDING.lock().ok().and_then(|mut map| map.as_mut().and_then(|m| m.remove(&id)));
    if let Some(sender) = sender {
        let _ = sender.send(allow);
    }
}

fn wait_for_answer(stream: &mut TcpStream, id: u64, rx: &mpsc::Receiver<bool>) -> Option<bool> {
    let _ = stream.set_nonblocking(true);
    let mut waited = 0;
    while waited < ASK_WAIT {
        if let Ok(allow) = rx.recv_timeout(Duration::from_secs(1)) {
            let _ = stream.set_nonblocking(false);
            return Some(allow);
        }
        waited += 1;
        let mut probe = [0u8; 1];
        if matches!(stream.peek(&mut probe), Ok(0)) {
            break;
        }
    }
    if let Ok(mut map) = PENDING.lock() {
        if let Some(map) = map.as_mut() {
            map.remove(&id);
        }
    }
    let _ = stream.set_nonblocking(false);
    None
}

fn handle(mut stream: TcpStream, app: &AppHandle, token: &str) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let mut buf = Vec::new();
    let mut chunk = [0u8; 1024];
    let head_end = loop {
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => return,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
        }
        if let Some(pos) = find(&buf, b"\r\n\r\n") {
            break pos + 4;
        }
        if buf.len() > MAX_HEAD {
            return reply(&mut stream, "431 Request Header Fields Too Large");
        }
    };

    let head = String::from_utf8_lossy(&buf[..head_end]).into_owned();
    let mut lines = head.split("\r\n");
    let request = lines.next().unwrap_or("");
    let ask = match request {
        "POST /activity HTTP/1.1" | "POST /activity HTTP/1.0" => false,
        "POST /ask HTTP/1.1" | "POST /ask HTTP/1.0" => true,
        _ => return reply(&mut stream, "404 Not Found"),
    };
    let (mut auth, mut length) = (String::new(), 0usize);
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            match name.trim().to_ascii_lowercase().as_str() {
                "authorization" => auth = value.trim().to_string(),
                "content-length" => length = value.trim().parse().unwrap_or(usize::MAX),
                _ => {}
            }
        }
    }
    if !same(&auth, &format!("Bearer {token}")) {
        return reply(&mut stream, "401 Unauthorized");
    }
    if length > MAX_BODY {
        return reply(&mut stream, "413 Payload Too Large");
    }
    let mut body = buf[head_end..].to_vec();
    while body.len() < length {
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => return,
            Ok(n) => body.extend_from_slice(&chunk[..n]),
        }
    }
    body.truncate(length);

    if ask {
        let Ok(incoming) = serde_json::from_slice::<IncomingAsk>(&body) else {
            return reply(&mut stream, "400 Bad Request");
        };
        let id = NEXT_ASK.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = mpsc::channel();
        if let Ok(mut map) = PENDING.lock() {
            map.get_or_insert_with(HashMap::new).insert(id, tx);
        }
        let (name, project, window) = (clean(incoming.name, 40), clean(incoming.project, 40), incoming.window);
        let _ = app.emit(
            "agent-activity",
            Activity {
                state: "needs_input",
                name: name.clone(),
                project: project.clone(),
                window,
                detail: None,
                ask: Some(Ask { id, tool: clean(incoming.tool, 40), detail: clean(incoming.detail, 200) }),
                cancelled: None,
            },
        );
        return match wait_for_answer(&mut stream, id, &rx) {
            Some(true) => reply_json(&mut stream, r#"{"decision":"allow"}"#),
            Some(false) => reply_json(&mut stream, r#"{"decision":"deny"}"#),
            None => {
                let _ = app.emit("agent-activity", Activity { state: "working", name, project, window, detail: None, ask: None, cancelled: Some(id) });
                reply(&mut stream, "204 No Content");
            }
        };
    }

    let Ok(incoming) = serde_json::from_slice::<Incoming>(&body) else {
        return reply(&mut stream, "400 Bad Request");
    };
    let Some(state) = state_of(&incoming.state) else {
        return reply(&mut stream, "400 Bad Request");
    };
    let activity = state.map(|state| Activity {
        state,
        name: clean(incoming.name, 40),
        project: clean(incoming.project, 40),
        window: incoming.window,
        detail: clean(incoming.detail, 200),
        ask: None,
        cancelled: None,
    });
    let _ = app.emit("agent-activity", activity);
    reply(&mut stream, "204 No Content");
}

fn hook_entries(exe: &str) -> Vec<(&'static str, Option<&'static str>, serde_json::Value)> {
    let hook = |args: &[&str], timeout: Option<u64>| {
        let mut all = vec!["agent"];
        all.extend_from_slice(args);
        all.extend_from_slice(&["--name", "Claude Code"]);
        let mut entry = serde_json::json!({ "type": "command", "command": exe, "args": all });
        if let Some(timeout) = timeout {
            entry["timeout"] = timeout.into();
        }
        entry
    };
    vec![
        ("UserPromptSubmit", None, hook(&["working"], None)),
        ("PreToolUse", None, hook(&["tool"], None)),
        ("PostToolUse", None, hook(&["working"], None)),
        ("PostToolUseFailure", None, hook(&["error"], None)),
        ("PermissionRequest", None, hook(&["ask"], Some(ASK_WAIT + 10))),
        ("Notification", Some("idle_prompt"), hook(&["needs-input"], None)),
        ("Stop", None, hook(&["done"], None)),
        ("SessionEnd", None, hook(&["clear"], None)),
    ]
}

fn is_ours(group: &serde_json::Value) -> bool {
    group["hooks"].as_array().is_some_and(|hooks| {
        hooks.iter().any(|hook| {
            hook["args"][0].as_str() == Some("agent") && hook["command"].as_str().is_some_and(|c| c.to_ascii_lowercase().contains("nectar"))
        })
    })
}

fn sync_claude_hooks(enable: bool) {
    let Some(path) = std::env::var_os("USERPROFILE").map(|d| PathBuf::from(d).join(".claude").join("settings.json")) else { return };
    let text = std::fs::read_to_string(&path).ok();
    let mut root: serde_json::Value = match text.as_deref().map(str::trim) {
        Some(t) if !t.is_empty() => match serde_json::from_str(t) {
            Ok(value) => value,
            Err(_) => return,
        },
        _ if enable => serde_json::json!({}),
        _ => return,
    };
    let original = root.clone();
    let Some(exe) = std::env::current_exe().ok().map(|p| p.to_string_lossy().into_owned()) else { return };
    let Some(object) = root.as_object_mut() else { return };

    let hooks = object.entry("hooks").or_insert_with(|| serde_json::json!({}));
    let Some(hooks) = hooks.as_object_mut() else { return };
    for event in hooks.values_mut() {
        if let Some(groups) = event.as_array_mut() {
            groups.retain(|group| !is_ours(group));
        }
    }
    if enable {
        for (event, matcher, hook) in hook_entries(&exe) {
            let mut group = serde_json::json!({ "hooks": [hook] });
            if let Some(matcher) = matcher {
                group["matcher"] = matcher.into();
            }
            match hooks.entry(event).or_insert_with(|| serde_json::json!([])).as_array_mut() {
                Some(groups) => groups.push(group),
                None => return,
            }
        }
    }
    hooks.retain(|_, event| event.as_array().map(|groups| !groups.is_empty()).unwrap_or(true));
    if hooks.is_empty() {
        object.remove("hooks");
    }
    if root == original {
        return;
    }

    let Ok(mut out) = serde_json::to_string_pretty(&root) else { return };
    out.push('\n');
    if let Some(text) = &text {
        let backup = path.with_file_name("settings.json.nectar-backup");
        if !backup.exists() {
            let _ = std::fs::write(&backup, text);
        }
    }
    let temp = path.with_file_name("settings.json.nectar-tmp");
    if std::fs::write(&temp, out).is_ok() && std::fs::rename(&temp, &path).is_err() {
        let _ = std::fs::remove_file(&temp);
    }
}

pub fn set_hooks(enable: bool) {
    let is_build_output = std::env::current_exe().is_ok_and(|p| p.components().any(|c| c.as_os_str() == "target"));
    if cfg!(debug_assertions) || is_build_output {
        return;
    }
    if enable && !std::env::var_os("USERPROFILE").is_some_and(|d| PathBuf::from(d).join(".claude").is_dir()) {
        return;
    }
    sync_claude_hooks(enable);
}

pub fn start(app: AppHandle) {
    let enabled = crate::utils::get_setting_str(&app, SETTING).as_deref() == Some("true");
    if !enabled {
        return;
    }
    std::thread::spawn(|| set_hooks(true));
    let Some(token) = random_token() else { return };
    let Ok(listener) = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0))) else { return };
    let Ok(addr) = listener.local_addr() else { return };
    let Some(path) = config_path() else { return };
    let config = serde_json::json!({ "port": addr.port(), "token": token }).to_string();
    if std::fs::write(&path, config).is_err() {
        return;
    }

    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { continue };
            if !stream.peer_addr().map(|a| a.ip().is_loopback()).unwrap_or(false) {
                continue;
            }
            if ACTIVE.fetch_add(1, Ordering::SeqCst) >= MAX_CONNECTIONS {
                ACTIVE.fetch_sub(1, Ordering::SeqCst);
                continue;
            }
            let app = app.clone();
            let token = token.clone();
            std::thread::spawn(move || {
                handle(stream, &app, &token);
                ACTIVE.fetch_sub(1, Ordering::SeqCst);
            });
        }
    });
}

fn project_name() -> Option<String> {
    let dir = std::env::var_os("CLAUDE_PROJECT_DIR").map(PathBuf::from).or_else(|| std::env::current_dir().ok())?;
    dir.file_name().map(|n| n.to_string_lossy().into_owned())
}

fn ancestor_window() -> Option<u64> {
    use windows::core::BOOL;
    use windows::Win32::Foundation::{HWND, LPARAM};
    use windows::Win32::System::Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS};
    use windows::Win32::UI::WindowsAndMessaging::{EnumWindows, GetWindow, GetWindowThreadProcessId, IsWindowVisible, GW_OWNER};

    let mut parents: HashMap<u32, (u32, String)> = HashMap::new();
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).ok()?;
        let mut entry = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        let mut more = Process32FirstW(snapshot, &mut entry).is_ok();
        while more {
            let len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
            parents.insert(entry.th32ProcessID, (entry.th32ParentProcessID, String::from_utf16_lossy(&entry.szExeFile[..len]).to_ascii_lowercase()));
            more = Process32NextW(snapshot, &mut entry).is_ok();
        }
        let _ = windows::Win32::Foundation::CloseHandle(snapshot);
    }

    let mut chain = Vec::new();
    let mut pid = std::process::id();
    while let Some((parent, _)) = parents.get(&pid) {
        match parents.get(parent) {
            Some((_, name)) if name != "explorer.exe" && !chain.contains(parent) => chain.push(*parent),
            _ => break,
        }
        pid = *parent;
    }

    struct Search {
        chain: Vec<u32>,
        found: Vec<(usize, u64)>,
    }
    unsafe extern "system" fn visit(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let search = &mut *(lparam.0 as *mut Search);
        if IsWindowVisible(hwnd).as_bool() && GetWindow(hwnd, GW_OWNER).map(|o| o.0.is_null()).unwrap_or(true) {
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if let Some(rank) = search.chain.iter().position(|&p| p == pid) {
                search.found.push((rank, hwnd.0 as usize as u64));
            }
        }
        BOOL(1)
    }
    let mut search = Search { chain, found: Vec::new() };
    unsafe {
        let _ = EnumWindows(Some(visit), LPARAM(&mut search as *mut Search as isize));
    }
    search.found.into_iter().min_by_key(|(rank, _)| *rank).map(|(_, hwnd)| hwnd)
}

fn flags(args: &[String]) -> (Option<String>, Option<String>) {
    let (mut name, mut project) = (None, None);
    let mut rest = args.iter();
    while let Some(flag) = rest.next() {
        match flag.as_str() {
            "--name" | "-n" => name = rest.next().cloned(),
            "--project" | "-p" => project = rest.next().cloned(),
            _ => {}
        }
    }
    (name, project.or_else(project_name))
}

fn post(path: &str, body: String, wait: u64) -> Option<String> {
    let config: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(config_path()?).ok()?).ok()?;
    let port = config.get("port")?.as_u64()? as u16;
    let token = config.get("token")?.as_str()?;

    let mut stream = TcpStream::connect_timeout(&SocketAddr::from((Ipv4Addr::LOCALHOST, port)), Duration::from_secs(1)).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(wait))).ok()?;
    write!(
        stream,
        "POST {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .ok()?;
    let mut response = String::new();
    stream.read_to_string(&mut response).ok()?;
    Some(response)
}

fn send(args: &[String]) -> Option<()> {
    send_with(args, None)
}

fn send_with(args: &[String], detail: Option<String>) -> Option<()> {
    let state = args.first()?.clone();
    let (name, project) = flags(&args[1..]);
    let body = serde_json::json!({ "state": state, "name": name, "project": project, "window": ancestor_window(), "detail": detail }).to_string();
    post("/activity", body, 2)?.starts_with("HTTP/1.1 204").then_some(())
}

fn ask(args: &[String]) {
    let (name, project) = flags(args);
    let mut input = String::new();
    let _ = std::io::stdin().read_to_string(&mut input);
    let event: serde_json::Value = serde_json::from_str(&input).unwrap_or_default();
    let tool_input = &event["tool_input"];
    if event["tool_name"].as_str() == Some("AskUserQuestion") {
        let question = tool_input["questions"][0]["question"].as_str().map(str::to_string);
        let mut forwarded = vec!["needs-input".to_string()];
        forwarded.extend(args.iter().cloned());
        let _ = send_with(&forwarded, question);
        return;
    }
    let detail = ["command", "file_path", "url", "pattern", "description"]
        .iter()
        .find_map(|key| tool_input.get(*key).and_then(|v| v.as_str()))
        .map(str::to_string)
        .or_else(|| (!tool_input.is_null()).then(|| tool_input.to_string()));
    let body = serde_json::json!({
        "name": name,
        "project": project,
        "window": ancestor_window(),
        "tool": event["tool_name"].as_str(),
        "detail": detail,
    })
    .to_string();

    let Some(response) = post("/ask", body, ASK_WAIT + 5) else { return };
    let decision = response.split("\r\n\r\n").nth(1).and_then(|b| serde_json::from_str::<serde_json::Value>(b).ok());
    let behavior = match decision.as_ref().and_then(|d| d["decision"].as_str()) {
        Some("allow") => serde_json::json!({ "behavior": "allow" }),
        Some("deny") => serde_json::json!({ "behavior": "deny", "message": "Denied from Nectar" }),
        _ => return,
    };
    println!("{}", serde_json::json!({ "hookSpecificOutput": { "hookEventName": "PermissionRequest", "decision": behavior } }));
}

pub fn run_cli(args: &[String]) -> Option<i32> {
    if args.get(1).map(String::as_str) != Some("agent") {
        return None;
    }
    match args.get(2).map(String::as_str) {
        Some("install-hooks") => {
            set_hooks(true);
            return Some(0);
        }
        Some("remove-hooks") => {
            set_hooks(false);
            return Some(0);
        }
        _ => {}
    }
    if args.get(2).map(String::as_str) == Some("ask") {
        ask(&args[3..]);
        return Some(0);
    }
    Some(if send(&args[2..]).is_some() { 0 } else { 1 })
}
