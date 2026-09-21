use tauri::Emitter;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitialState {
    pub uninstall_mode: bool,
    pub default_install_dir_user: String,
    pub default_install_dir_all_users: String,
    pub prefill_install_dir: Option<String>,
    pub prefill_all_users: bool,
    pub prefill_desktop_shortcut: bool,
    pub auto_install: bool,
    pub app_version: String,
    pub payload_present: bool,
}

#[tauri::command]
pub fn get_initial_state() -> InitialState {
    let launch = crate::state::parse_launch_args();
    InitialState {
        uninstall_mode: launch.uninstall,
        default_install_dir_user: crate::known_folders::default_install_dir(false)
            .to_string_lossy()
            .to_string(),
        default_install_dir_all_users: crate::known_folders::default_install_dir(true)
            .to_string_lossy()
            .to_string(),
        prefill_install_dir: launch.prefill_install_dir,
        prefill_all_users: launch.prefill_all_users,
        prefill_desktop_shortcut: launch.prefill_desktop_shortcut,
        auto_install: launch.auto_install,
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        payload_present: crate::payload::payload_present(),
    }
}

#[tauri::command]
pub async fn pick_install_dir(app: tauri::AppHandle, current: String) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    let (tx, rx) = std::sync::mpsc::channel();
    app.dialog()
        .file()
        .set_directory(&current)
        .pick_folder(move |folder| {
            let _ = tx.send(folder);
        });
    rx.recv().ok().flatten().map(|p| p.to_string())
}

#[tauri::command]
pub fn needs_elevation(all_users: bool) -> bool {
    all_users && !crate::elevate::is_elevated()
}

#[tauri::command]
pub fn relaunch_elevated(
    app: tauri::AppHandle,
    install_dir: String,
    all_users: bool,
    desktop_shortcut: bool,
) -> Result<(), String> {
    let mut args = vec![format!("--install-dir=\"{}\"", install_dir), "--auto-install".to_string()];
    if all_users {
        args.push("--all-users".to_string());
    }
    if !desktop_shortcut {
        args.push("--no-desktop-shortcut".to_string());
    }
    crate::elevate::relaunch_elevated(&args).map_err(|e| e.to_string())?;
    app.exit(0);
    Ok(())
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ProgressEvent {
    stage: String,
    percent: f32,
    message: String,
}

fn dir_size_kb(dir: &std::path::Path) -> u32 {
    let mut total: u64 = 0;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                total += meta.len();
            }
        }
    }
    (total / 1024) as u32
}

#[tauri::command]
pub async fn start_install(
    app: tauri::AppHandle,
    install_dir: String,
    all_users: bool,
    desktop_shortcut: bool,
) -> Result<String, String> {
    if all_users && !crate::elevate::is_elevated() {
        return Err("elevation_required".to_string());
    }
    if !crate::payload::payload_present() {
        return Err("This installer was built without a bundled app payload.".to_string());
    }

    let install_path = std::path::PathBuf::from(&install_dir);

    let app_extract = app.clone();
    let install_path_extract = install_path.clone();
    tauri::async_runtime::spawn_blocking(move || {
        crate::payload::extract(&install_path_extract, |p, msg| {
            let _ = app_extract.emit(
                "install-progress",
                ProgressEvent { stage: "extracting".into(), percent: p * 0.7, message: msg.to_string() },
            );
        })
    })
    .await
    .map_err(|e| e.to_string())??;

    let _ = app.emit(
        "install-progress",
        ProgressEvent { stage: "shortcuts".into(), percent: 0.75, message: "Creating shortcuts...".into() },
    );

    let exe_path = crate::payload::app_exe_path(&install_path);
    let uninstall_exe = install_path.join("uninstall.exe");

    if let Some(dir) = crate::known_folders::start_menu_programs_dir(all_users) {
        let lnk = dir.join("Nectar.lnk");
        let _ = crate::shortcuts::create_shortcut(&lnk, &exe_path, "", &exe_path, "Nectar");
    }
    if desktop_shortcut {
        if let Some(dir) = crate::known_folders::desktop_dir(all_users) {
            let lnk = dir.join("Nectar.lnk");
            let _ = crate::shortcuts::create_shortcut(&lnk, &exe_path, "", &exe_path, "Nectar");
        }
    }

    let _ = app.emit(
        "install-progress",
        ProgressEvent { stage: "registry".into(), percent: 0.88, message: "Registering with Windows...".into() },
    );

    let self_exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let _ = std::fs::copy(&self_exe, &uninstall_exe);

    // Uninstall needs to know its own install scope (HKLM vs HKCU) later, without
    // being able to infer it reliably from the path alone.
    let _ = std::fs::write(
        install_path.join(".install-scope"),
        if all_users { "all-users" } else { "user" },
    );

    let size_kb = dir_size_kb(&install_path);
    let uninstall_string = format!("\"{}\" --uninstall", uninstall_exe.to_string_lossy());
    let install_location = install_path.to_string_lossy().to_string();
    let icon_str = exe_path.to_string_lossy().to_string();
    let info = crate::registry::UninstallInfo {
        display_name: "Nectar",
        display_icon: &icon_str,
        display_version: env!("CARGO_PKG_VERSION"),
        publisher: "kua8",
        install_location: &install_location,
        uninstall_string: &uninstall_string,
        estimated_size_kb: size_kb,
    };
    // Unlike the shortcuts above, a failure here is fatal and must surface: this
    // registry entry is the only thing that makes Nectar show up in Add/Remove
    // Programs at all. Swallowing this error (as it was before) let all-users
    // installs silently finish "successfully" with files on disk but no way to
    // uninstall them through Windows — exactly the orphaned-install bug this
    // fixes.
    crate::registry::write_uninstall_entry(all_users, &info)
        .map_err(|e| format!("Files were installed, but registering with Windows failed: {e}. Try running the installer as Administrator."))?;

    let _ = app.emit(
        "install-progress",
        ProgressEvent { stage: "done".into(), percent: 1.0, message: "Done".into() },
    );

    Ok(exe_path.to_string_lossy().to_string())
}

#[tauri::command]
pub fn launch_app(exe_path: String) -> Result<(), String> {
    std::process::Command::new(&exe_path).spawn().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn open_install_folder(path: String) -> Result<(), String> {
    std::process::Command::new("explorer").arg(&path).spawn().map_err(|e| e.to_string())?;
    Ok(())
}

/// Opens a URL in the default browser — used for the optional uninstall-feedback
/// link (a pre-filled GitHub issue) so submitting feedback is an explicit, visible
/// action the person takes in their own browser, not something the installer sends.
/// Uses ShellExecuteW's "open" verb rather than spawning explorer.exe directly —
/// the same pattern already used for opening ms-settings:/ms-availablenetworks:
/// links elsewhere in this codebase; explorer.exe as a plain child process is not
/// reliable for arbitrary URLs (in particular ones with query strings) on current
/// Windows builds.
#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    unsafe {
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
        use windows::core::PCWSTR;

        let open_w: Vec<u16> = "open".encode_utf16().chain(std::iter::once(0)).collect();
        let url_w: Vec<u16> = url.encode_utf16().chain(std::iter::once(0)).collect();

        let result = ShellExecuteW(
            None,
            PCWSTR(open_w.as_ptr()),
            PCWSTR(url_w.as_ptr()),
            None,
            None,
            SW_SHOWNORMAL,
        );

        if (result.0 as isize) <= 32 {
            return Err(format!("Could not open browser (error {})", result.0 as isize));
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn start_uninstall(app: tauri::AppHandle) -> Result<(), String> {
    let self_exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let install_dir = self_exe.parent().ok_or("no parent dir")?.to_path_buf();

    let _ = app.emit(
        "uninstall-progress",
        ProgressEvent { stage: "removing".into(), percent: 0.3, message: "Removing shortcuts...".into() },
    );

    let mut shortcuts = Vec::new();
    for all_users in [false, true] {
        if let Some(dir) = crate::known_folders::start_menu_programs_dir(all_users) {
            shortcuts.push(dir.join("Nectar.lnk"));
        }
        if let Some(dir) = crate::known_folders::desktop_dir(all_users) {
            shortcuts.push(dir.join("Nectar.lnk"));
        }
    }

    let _ = app.emit(
        "uninstall-progress",
        ProgressEvent { stage: "removing".into(), percent: 0.6, message: "Removing registry entries...".into() },
    );

    let scope = std::fs::read_to_string(install_dir.join(".install-scope")).unwrap_or_default();
    let all_users_install = scope.trim() == "all-users";

    crate::uninstall::uninstall(&install_dir, all_users_install, &shortcuts)?;

    let _ = app.emit(
        "uninstall-progress",
        ProgressEvent { stage: "done".into(), percent: 1.0, message: "Done".into() },
    );

    Ok(())
}

#[tauri::command]
pub fn window_minimize(window: tauri::WebviewWindow) {
    let _ = window.minimize();
}

#[tauri::command]
pub fn window_close(window: tauri::WebviewWindow) {
    let _ = window.close();
}
