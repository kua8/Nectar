use std::path::PathBuf;
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::UI::Shell::{
    SHGetKnownFolderPath, FOLDERID_CommonStartMenu, FOLDERID_Desktop, FOLDERID_ProgramFilesX64,
    FOLDERID_PublicDesktop, FOLDERID_StartMenu, FOLDERID_UserProgramFiles, KF_FLAG_CREATE,
};

fn known_folder(rfid: *const windows::core::GUID) -> Option<PathBuf> {
    unsafe {
        let pwstr = SHGetKnownFolderPath(rfid, KF_FLAG_CREATE, None).ok()?;
        let path = pwstr.to_string().ok()?;
        CoTaskMemFree(Some(pwstr.0 as *const _));
        Some(PathBuf::from(path))
    }
}

pub fn program_files_dir() -> Option<PathBuf> {
    known_folder(&FOLDERID_ProgramFilesX64)
}

pub fn user_programs_dir() -> Option<PathBuf> {
    known_folder(&FOLDERID_UserProgramFiles)
}

pub fn desktop_dir(all_users: bool) -> Option<PathBuf> {
    known_folder(if all_users {
        &FOLDERID_PublicDesktop
    } else {
        &FOLDERID_Desktop
    })
}

/// FOLDERID_(Common)StartMenu already resolves to "...\Start Menu\Programs", the
/// folder Explorer actually reads app shortcuts from — not the "Start Menu" folder
/// one level up.
pub fn start_menu_programs_dir(all_users: bool) -> Option<PathBuf> {
    known_folder(if all_users {
        &FOLDERID_CommonStartMenu
    } else {
        &FOLDERID_StartMenu
    })
}

pub fn default_install_dir(all_users: bool) -> PathBuf {
    if all_users {
        program_files_dir()
            .unwrap_or_else(|| PathBuf::from(r"C:\Program Files"))
            .join("Nectar")
    } else {
        user_programs_dir()
            .unwrap_or_else(|| PathBuf::from(r"C:\Users\Default\AppData\Local\Programs"))
            .join("Nectar")
    }
}
