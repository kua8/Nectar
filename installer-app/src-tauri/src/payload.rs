use std::io::Cursor;
use std::path::{Path, PathBuf};

static PAYLOAD: &[u8] = include_bytes!("../payload.zip");

pub fn payload_present() -> bool {
    !PAYLOAD.is_empty()
}

pub fn extract<F: FnMut(f32, &str)>(dest: &Path, mut on_progress: F) -> Result<(), String> {
    let reader = Cursor::new(PAYLOAD);
    let mut archive = zip::ZipArchive::new(reader).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(dest).map_err(|e| e.to_string())?;

    let total = archive.len().max(1);
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let out_path = match entry.enclosed_name() {
            Some(p) => dest.join(p),
            None => continue,
        };

        if entry.is_dir() {
            std::fs::create_dir_all(&out_path).map_err(|e| e.to_string())?;
        } else {
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let mut out_file = std::fs::File::create(&out_path).map_err(|e| e.to_string())?;
            std::io::copy(&mut entry, &mut out_file).map_err(|e| e.to_string())?;
        }

        on_progress((i + 1) as f32 / total as f32, &out_path.to_string_lossy());
    }

    Ok(())
}

pub fn app_exe_path(install_dir: &Path) -> PathBuf {
    install_dir.join("nectar.exe")
}
