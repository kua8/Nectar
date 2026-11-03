use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

const MAX_HEAD: usize = 8192;
const MAX_BODY: usize = 4096;
const MAX_CONNECTIONS: usize = 8;
const SETTING: &str = "nectar-agent-api";

static ACTIVE: AtomicUsize = AtomicUsize::new(0);

#[derive(Deserialize)]
struct Incoming {
    state: String,
    name: Option<String>,
    project: Option<String>,
}

#[derive(Serialize, Clone)]
struct Activity {
    state: &'static str,
    name: Option<String>,
    project: Option<String>,
}

fn config_path() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|dir| PathBuf::from(dir).join("com.kua8.nectar").join("agent.json"))
}

fn state_of(raw: &str) -> Option<Option<&'static str>> {
    match raw.trim().to_ascii_lowercase().replace('-', "_").as_str() {
        "working" | "thinking" | "running" => Some(Some("working")),
        "needs_input" | "input" | "waiting" => Some(Some("needs_input")),
        "done" | "completed" | "complete" | "finished" => Some(Some("done")),
        "error" | "failed" => Some(Some("error")),
        "clear" | "idle" | "stop" => Some(None),
        _ => None,
    }
}

fn clean(text: Option<String>) -> Option<String> {
    let cleaned: String = text?.chars().filter(|c| !c.is_control()).take(40).collect();
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

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
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
    if request != "POST /activity HTTP/1.1" && request != "POST /activity HTTP/1.0" {
        return reply(&mut stream, "404 Not Found");
    }
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

    let Ok(incoming) = serde_json::from_slice::<Incoming>(&body) else {
        return reply(&mut stream, "400 Bad Request");
    };
    let Some(state) = state_of(&incoming.state) else {
        return reply(&mut stream, "400 Bad Request");
    };
    let activity = state.map(|state| Activity { state, name: clean(incoming.name), project: clean(incoming.project) });
    let _ = app.emit("agent-activity", activity);
    reply(&mut stream, "204 No Content");
}

pub fn start(app: AppHandle) {
    if crate::utils::get_setting_str(&app, SETTING).as_deref() != Some("true") {
        return;
    }
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

fn send(args: &[String]) -> Option<()> {
    let state = args.first()?.clone();
    let (mut name, mut project) = (None, None);
    let mut rest = args[1..].iter();
    while let Some(flag) = rest.next() {
        match flag.as_str() {
            "--name" | "-n" => name = rest.next().cloned(),
            "--project" | "-p" => project = rest.next().cloned(),
            _ => {}
        }
    }
    let project = project.or_else(project_name);

    let config: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(config_path()?).ok()?).ok()?;
    let port = config.get("port")?.as_u64()? as u16;
    let token = config.get("token")?.as_str()?;

    let body = serde_json::json!({ "state": state, "name": name, "project": project }).to_string();
    let mut stream = TcpStream::connect_timeout(&SocketAddr::from((Ipv4Addr::LOCALHOST, port)), Duration::from_secs(1)).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(2))).ok()?;
    write!(
        stream,
        "POST /activity HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .ok()?;
    let mut response = String::new();
    stream.read_to_string(&mut response).ok()?;
    response.starts_with("HTTP/1.1 204").then_some(())
}

pub fn run_cli(args: &[String]) -> Option<i32> {
    if args.get(1).map(String::as_str) != Some("agent") {
        return None;
    }
    Some(if send(&args[2..]).is_some() { 0 } else { 1 })
}
