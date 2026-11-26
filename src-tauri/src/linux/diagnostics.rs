//! Small append-only log at ~/.local/share/com.kua8.nectar/nectar.log.
use std::io::Write;
use std::sync::Mutex;

static LOG_PATH: Mutex<Option<std::path::PathBuf>> = Mutex::new(None);

pub fn init(dir: std::path::PathBuf) {
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("nectar.log");
    // Keep the log small: start over once it passes 1 MB.
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > 1024 * 1024) {
        let _ = std::fs::remove_file(&path);
    }
    if let Ok(mut p) = LOG_PATH.lock() {
        *p = Some(path);
    }
}

pub fn log_dir() -> Option<std::path::PathBuf> {
    LOG_PATH.lock().ok()?.as_ref()?.parent().map(|p| p.to_path_buf())
}

pub fn log(msg: &str) {
    let line = format!("[{}] {}\n", chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"), msg);
    if cfg!(debug_assertions) {
        eprint!("{line}");
    }
    let Ok(guard) = LOG_PATH.lock() else { return };
    if let Some(path) = guard.as_ref() {
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
            let _ = f.write_all(line.as_bytes());
        }
    }
}

pub fn read_tail(max_bytes: usize) -> String {
    let Some(path) = LOG_PATH.lock().ok().and_then(|g| g.clone()) else { return String::new() };
    let content = std::fs::read_to_string(path).unwrap_or_default();
    if content.len() > max_bytes {
        let mut start = content.len() - max_bytes;
        while !content.is_char_boundary(start) {
            start += 1;
        }
        content[start..].to_string()
    } else {
        content
    }
}
