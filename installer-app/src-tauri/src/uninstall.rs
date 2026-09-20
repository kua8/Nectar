use std::path::{Path, PathBuf};

/// Removes shortcuts and the registry entry, then hands off directory deletion to a
/// detached `cmd` — the running uninstall.exe can't delete its own file while it's
/// still executing, so the shell waits a beat after we exit and removes everything,
/// itself included, in one shot.
pub fn uninstall(install_dir: &Path, all_users: bool, shortcuts: &[PathBuf]) -> Result<(), String> {
    for s in shortcuts {
        let _ = std::fs::remove_file(s);
    }

    let _ = crate::registry::remove_uninstall_entry(all_users);

    let dir_str = install_dir.to_string_lossy().to_string();
    let inner = format!(
        "timeout /T 1 /NOBREAK >NUL & rmdir /S /Q \"{}\"",
        dir_str
    );
    std::process::Command::new("cmd")
        .arg("/C")
        .arg(inner)
        .spawn()
        .map_err(|e| e.to_string())?;

    Ok(())
}
