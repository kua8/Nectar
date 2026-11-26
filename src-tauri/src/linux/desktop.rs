//! Freedesktop .desktop entries: the installed app list, matching running windows
//! to their app, icon theme lookup and launching.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

#[derive(Clone, Debug)]
pub struct DesktopEntry {
    /// Desktop file id without ".desktop", e.g. "org.gnome.Nautilus".
    pub id: String,
    pub path: String,
    pub name: String,
    pub icon: Option<String>,
    pub exec: Option<String>,
    pub wm_class: Option<String>,
    pub no_display: bool,
    pub terminal: bool,
}

impl DesktopEntry {
    /// Basename of the program in Exec, skipping env/flatpak wrappers.
    pub fn exec_binary(&self) -> Option<String> {
        let exec = self.exec.as_deref()?;
        let mut words = split_exec(exec).into_iter();
        let mut first = words.next()?;
        if first == "env" {
            first = words.find(|w| !w.contains('='))?;
        }
        if first.ends_with("flatpak") {
            return words.find(|w| !w.starts_with('-') && w != "run").map(|w| w.to_lowercase());
        }
        Path::new(&first).file_name().map(|f| f.to_string_lossy().to_lowercase())
    }
}

pub fn data_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let home = std::env::var_os("HOME").map(PathBuf::from);
    match std::env::var_os("XDG_DATA_HOME") {
        Some(d) if !d.is_empty() => dirs.push(PathBuf::from(d)),
        _ => {
            if let Some(h) = &home {
                dirs.push(h.join(".local/share"));
            }
        }
    }
    let system = std::env::var("XDG_DATA_DIRS").ok().filter(|s| !s.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    for d in system.split(':') {
        dirs.push(PathBuf::from(d));
    }
    for extra in ["/var/lib/flatpak/exports/share", "/var/lib/snapd/desktop"] {
        let p = PathBuf::from(extra);
        if !dirs.contains(&p) {
            dirs.push(p);
        }
    }
    if let Some(h) = &home {
        let p = h.join(".local/share/flatpak/exports/share");
        if !dirs.contains(&p) {
            dirs.push(p);
        }
    }
    dirs
}

fn current_desktops() -> Vec<String> {
    std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .split(':')
        .map(|s| s.to_lowercase())
        .filter(|s| !s.is_empty())
        .collect()
}

fn locale_keys() -> Vec<String> {
    let lang = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .filter_map(|k| std::env::var(k).ok())
        .find(|v| !v.is_empty() && v != "C" && v != "POSIX")
        .unwrap_or_default();
    let base = lang.split('.').next().unwrap_or("").to_string();
    let short = base.split('_').next().unwrap_or("").to_string();
    let mut keys = Vec::new();
    if !base.is_empty() {
        keys.push(format!("Name[{base}]"));
    }
    if !short.is_empty() && short != base {
        keys.push(format!("Name[{short}]"));
    }
    keys
}

pub fn parse(path: &Path, id: String) -> Option<DesktopEntry> {
    let content = std::fs::read_to_string(path).ok()?;
    let mut in_main = false;
    let mut fields: HashMap<String, String> = HashMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_main = line == "[Desktop Entry]";
            continue;
        }
        if !in_main || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            fields.entry(k.trim().to_string()).or_insert_with(|| v.trim().to_string());
        }
    }
    if fields.get("Type").map(String::as_str) != Some("Application") {
        return None;
    }
    let truthy = |k: &str| fields.get(k).is_some_and(|v| v == "true");
    let desktops = current_desktops();
    let mut hidden = truthy("Hidden") || truthy("NoDisplay");
    if let Some(only) = fields.get("OnlyShowIn") {
        if !only.split(';').any(|d| desktops.contains(&d.to_lowercase())) {
            hidden = true;
        }
    }
    if let Some(not) = fields.get("NotShowIn") {
        if not.split(';').any(|d| desktops.contains(&d.to_lowercase())) {
            hidden = true;
        }
    }
    let name = locale_keys()
        .iter()
        .find_map(|k| fields.get(k).cloned())
        .or_else(|| fields.get("Name").cloned())?;
    Some(DesktopEntry {
        id,
        path: path.to_string_lossy().to_string(),
        name,
        icon: fields.get("Icon").cloned().filter(|s| !s.is_empty()),
        exec: fields.get("Exec").cloned(),
        wm_class: fields.get("StartupWMClass").cloned().filter(|s| !s.is_empty()),
        no_display: hidden,
        terminal: truthy("Terminal"),
    })
}

fn scan() -> Vec<DesktopEntry> {
    let mut seen: HashMap<String, DesktopEntry> = HashMap::new();
    for dir in data_dirs() {
        let apps = dir.join("applications");
        walk(&apps, &apps, &mut seen);
    }
    let mut list: Vec<DesktopEntry> = seen.into_values().collect();
    list.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    list
}

fn walk(base: &Path, dir: &Path, seen: &mut HashMap<String, DesktopEntry>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(base, &p, seen);
            continue;
        }
        if p.extension().and_then(|x| x.to_str()) != Some("desktop") {
            continue;
        }
        // Desktop file ids use '-' for subdirectories.
        let rel = p.strip_prefix(base).unwrap_or(&p).to_string_lossy().replace('/', "-");
        let id = rel.trim_end_matches(".desktop").to_string();
        // Earlier data dirs win (user overrides system).
        if seen.contains_key(&id) {
            continue;
        }
        if let Some(entry) = parse(&p, id.clone()) {
            seen.insert(id, entry);
        }
    }
}

fn cache() -> &'static Mutex<Option<(std::time::Instant, Vec<DesktopEntry>)>> {
    static CACHE: OnceLock<Mutex<Option<(std::time::Instant, Vec<DesktopEntry>)>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

/// All desktop entries, rescanned at most every 30 s.
pub fn entries() -> Vec<DesktopEntry> {
    if let Ok(guard) = cache().lock() {
        if let Some((at, list)) = guard.as_ref() {
            if at.elapsed() < std::time::Duration::from_secs(30) {
                return list.clone();
            }
        }
    }
    let list = scan();
    if let Ok(mut guard) = cache().lock() {
        *guard = Some((std::time::Instant::now(), list.clone()));
    }
    list
}

pub fn invalidate() {
    if let Ok(mut guard) = cache().lock() {
        *guard = None;
    }
}

pub fn by_path(path: &str) -> Option<DesktopEntry> {
    if let Some(e) = entries().into_iter().find(|e| e.path == path) {
        return Some(e);
    }
    let p = Path::new(path);
    if p.extension().and_then(|x| x.to_str()) == Some("desktop") && p.is_file() {
        let id = p.file_stem()?.to_string_lossy().to_string();
        return parse(p, id);
    }
    None
}

/// Finds the desktop entry for a running window from its WM_CLASS, GTK app id and binary.
pub fn match_window(instance: &str, class: &str, gtk_id: Option<&str>, exe: Option<&str>) -> Option<DesktopEntry> {
    let all = entries();
    let lc = |s: &str| s.to_lowercase();
    let (inst, cls) = (lc(instance), lc(class));
    let exe_name = exe.and_then(|e| Path::new(e).file_name()).map(|f| f.to_string_lossy().to_lowercase());

    if let Some(id) = gtk_id {
        if let Some(e) = all.iter().find(|e| e.id.eq_ignore_ascii_case(id)) {
            return Some(e.clone());
        }
    }
    let wm_match = |e: &&DesktopEntry| e.wm_class.as_deref().is_some_and(|w| { let w = lc(w); w == cls || w == inst });
    if let Some(e) = all.iter().filter(|e| !e.no_display).find(wm_match).or_else(|| all.iter().find(wm_match)) {
        return Some(e.clone());
    }
    let id_match = |e: &&DesktopEntry| {
        let id = lc(&e.id);
        let last = id.rsplit('.').next().unwrap_or(&id).to_string();
        (!cls.is_empty() && (id == cls || last == cls)) || (!inst.is_empty() && (id == inst || last == inst))
    };
    if let Some(e) = all.iter().filter(|e| !e.no_display).find(id_match).or_else(|| all.iter().find(id_match)) {
        return Some(e.clone());
    }
    let exec_match = |e: &&DesktopEntry| {
        e.exec_binary().is_some_and(|b| b == inst || b == cls || exe_name.as_deref() == Some(b.as_str()))
    };
    all.iter().filter(|e| !e.no_display).find(exec_match).or_else(|| all.iter().find(exec_match)).cloned()
}

// ---------------------------------------------------------------- icons

fn icon_theme() -> String {
    static THEME: OnceLock<String> = OnceLock::new();
    THEME
        .get_or_init(|| {
            for schema in ["org.cinnamon.desktop.interface", "org.gnome.desktop.interface", "org.mate.interface"] {
                if let Some(v) = crate::utils::run("gsettings", &["get", schema, "icon-theme"]) {
                    let v = v.trim_matches('\'').to_string();
                    if !v.is_empty() {
                        return v;
                    }
                }
            }
            "hicolor".into()
        })
        .clone()
}

fn theme_inherits(theme_dir: &Path) -> Vec<String> {
    std::fs::read_to_string(theme_dir.join("index.theme"))
        .ok()
        .and_then(|c| {
            c.lines()
                .find(|l| l.starts_with("Inherits="))
                .map(|l| l["Inherits=".len()..].split(',').map(|s| s.trim().to_string()).collect())
        })
        .unwrap_or_default()
}

fn icon_base_dirs() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(h) = std::env::var_os("HOME").map(PathBuf::from) {
        v.push(h.join(".icons"));
    }
    for d in data_dirs() {
        v.push(d.join("icons"));
    }
    v
}

fn find_in_theme(theme: &str, name: &str, size: u32) -> Option<PathBuf> {
    let mut best: Option<(i64, PathBuf)> = None;
    for base in icon_base_dirs() {
        let tdir = base.join(theme);
        let Ok(subdirs) = std::fs::read_dir(&tdir) else { continue };
        for sd in subdirs.flatten() {
            let sd_path = sd.path();
            if !sd_path.is_dir() {
                continue;
            }
            let sd_name = sd.file_name().to_string_lossy().to_string();
            // Layouts are "48x48/apps", "apps/48", "scalable/apps", "48/apps".
            let mut stack = vec![sd_path.clone()];
            while let Some(dir) = stack.pop() {
                let Ok(files) = std::fs::read_dir(&dir) else { continue };
                for f in files.flatten() {
                    let fp = f.path();
                    if fp.is_dir() {
                        if stack.len() < 8 {
                            stack.push(fp);
                        }
                        continue;
                    }
                    let fname = f.file_name().to_string_lossy().to_string();
                    let (stem, ext) = match fname.rsplit_once('.') {
                        Some(x) => x,
                        None => continue,
                    };
                    if stem != name || !matches!(ext, "png" | "svg" | "xpm") {
                        continue;
                    }
                    let path_str = fp.to_string_lossy();
                    let dir_size = if path_str.contains("scalable") || ext == "svg" {
                        10_000
                    } else {
                        path_str
                            .split('/')
                            .filter_map(|seg| seg.split(['x', '@']).next().and_then(|n| n.parse::<i64>().ok()))
                            .last()
                            .unwrap_or_else(|| sd_name.parse().unwrap_or(48))
                    };
                    // Prefer exact or larger sizes, then the closest.
                    let score = if dir_size == 10_000 {
                        500
                    } else if dir_size >= size as i64 {
                        dir_size - size as i64
                    } else {
                        1000 + (size as i64 - dir_size)
                    };
                    if best.as_ref().is_none_or(|(s, _)| score < *s) {
                        best = Some((score, fp));
                    }
                }
            }
        }
        if best.is_some() {
            break;
        }
    }
    best.map(|(_, p)| p)
}

/// Resolves an Icon= value (name or absolute path) to a file.
pub fn resolve_icon(icon: &str, size: u32) -> Option<PathBuf> {
    let p = Path::new(icon);
    if p.is_absolute() {
        return p.is_file().then(|| p.to_path_buf());
    }
    let theme = icon_theme();
    let mut themes = vec![theme.clone()];
    for base in icon_base_dirs() {
        for t in theme_inherits(&base.join(&theme)) {
            if !themes.contains(&t) {
                themes.push(t);
            }
        }
    }
    for fallback in ["Mint-Y", "Adwaita", "hicolor"] {
        if !themes.iter().any(|t| t == fallback) {
            themes.push(fallback.into());
        }
    }
    for t in &themes {
        if let Some(found) = find_in_theme(t, icon, size) {
            return Some(found);
        }
    }
    for ext in ["png", "svg", "xpm"] {
        let p = PathBuf::from(format!("/usr/share/pixmaps/{icon}.{ext}"));
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

pub fn icon_data_url(icon: &str, size: u32) -> Option<String> {
    let path = resolve_icon(icon, size)?;
    crate::utils::image_file_to_data_url(&path, size)
}

// ---------------------------------------------------------------- launching

/// Splits an Exec line into words, honoring double quotes.
pub fn split_exec(exec: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    let mut chars = exec.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => in_quotes = !in_quotes,
            '\\' if in_quotes => {
                if let Some(n) = chars.next() {
                    cur.push(n);
                }
            }
            ' ' | '\t' if !in_quotes => {
                if !cur.is_empty() {
                    words.push(std::mem::take(&mut cur));
                }
            }
            _ => cur.push(c),
        }
    }
    if !cur.is_empty() {
        words.push(cur);
    }
    words
}

fn exec_command(entry: &DesktopEntry, extra: Option<&str>) -> Option<Vec<String>> {
    let exec = entry.exec.as_deref()?;
    let mut out = Vec::new();
    for w in split_exec(exec) {
        match w.as_str() {
            "%f" | "%F" | "%u" | "%U" | "%d" | "%D" | "%n" | "%N" | "%v" | "%m" | "%k" => {}
            "%i" => {
                if let Some(icon) = &entry.icon {
                    out.push("--icon".into());
                    out.push(icon.clone());
                }
            }
            "%c" => out.push(entry.name.clone()),
            _ => out.push(w.replace("%%", "%")),
        }
    }
    if let Some(extra) = extra.filter(|a| !a.trim().is_empty()) {
        out.extend(split_exec(extra));
    }
    if entry.terminal {
        let term = ["x-terminal-emulator", "gnome-terminal", "konsole", "xterm"]
            .into_iter()
            .find(|t| crate::utils::which(t))?;
        let mut wrapped = vec![term.to_string(), "-e".into()];
        wrapped.extend(out);
        return Some(wrapped);
    }
    (!out.is_empty()).then_some(out)
}

/// Launches a desktop entry, a program path or a command name.
pub fn launch(path: &str, args: Option<&str>) -> bool {
    let has_args = args.is_some_and(|a| !a.trim().is_empty());
    if path.ends_with(".desktop") {
        if let Some(entry) = by_path(path) {
            if !has_args && crate::utils::which("gio") && crate::utils::spawn_detached("gio", &["launch", &entry.path]) {
                return true;
            }
            if let Some(cmd) = exec_command(&entry, args) {
                let refs: Vec<&str> = cmd.iter().map(String::as_str).collect();
                return crate::utils::spawn_detached(refs[0], &refs[1..]);
            }
        }
        return false;
    }
    let mut argv: Vec<String> = vec![path.to_string()];
    if let Some(a) = args.filter(|a| !a.trim().is_empty()) {
        argv.extend(split_exec(a));
    }
    let p = Path::new(path);
    if p.is_dir() || (p.is_file() && !is_executable(p)) {
        return crate::utils::spawn_detached("xdg-open", &[path]);
    }
    let refs: Vec<&str> = argv.iter().map(String::as_str).collect();
    crate::utils::spawn_detached(refs[0], &refs[1..])
}

fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(p).is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
}

/// First entry whose id is one of the candidates, for default pins.
pub fn first_existing(ids: &[&str]) -> Option<DesktopEntry> {
    let all = entries();
    ids.iter().find_map(|id| all.iter().find(|e| e.id.eq_ignore_ascii_case(id)).cloned())
}

/// The user's default handler for a MIME type or URL scheme.
pub fn default_for(mime: &str) -> Option<DesktopEntry> {
    let id = crate::utils::run("xdg-mime", &["query", "default", mime])?;
    let id = id.trim().trim_end_matches(".desktop").to_string();
    entries().into_iter().find(|e| e.id == id)
}
