#[derive(Clone)]
pub struct LaunchArgs {
    pub uninstall: bool,
    pub prefill_install_dir: Option<String>,
    pub prefill_all_users: bool,
    pub prefill_desktop_shortcut: bool,
    pub auto_install: bool,
    pub auto_uninstall: bool,
}

pub fn parse_launch_args() -> LaunchArgs {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let launched_as_uninstaller = std::env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase()))
        .is_some_and(|n| n == "uninstall.exe");
    let mut out = LaunchArgs {
        uninstall: launched_as_uninstaller,
        prefill_install_dir: None,
        prefill_all_users: false,
        prefill_desktop_shortcut: true,
        auto_install: false,
        auto_uninstall: false,
    };
    for arg in &args {
        if arg == "--uninstall" {
            out.uninstall = true;
        } else if arg == "--all-users" {
            out.prefill_all_users = true;
        } else if arg == "--auto-uninstall" {
            out.auto_uninstall = true;
        } else if arg == "--auto-install" {
            out.auto_install = true;
        } else if arg == "--no-desktop-shortcut" {
            out.prefill_desktop_shortcut = false;
        } else if let Some(v) = arg.strip_prefix("--install-dir=") {
            out.prefill_install_dir = Some(v.trim_matches('"').to_string());
        }
    }
    out
}
