#[derive(Clone)]
pub struct LaunchArgs {
    pub uninstall: bool,
    pub prefill_install_dir: Option<String>,
    pub prefill_all_users: bool,
    pub prefill_desktop_shortcut: bool,
    pub auto_install: bool,
}

/// Parsed once per process. `--install-dir=`/`--all-users`/`--no-desktop-shortcut`/
/// `--auto-install` are set by `relaunch_elevated` so the elevated relaunch can resume
/// exactly where the pre-elevation instance left off, instead of making the user
/// re-pick everything after the UAC prompt.
pub fn parse_launch_args() -> LaunchArgs {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut out = LaunchArgs {
        uninstall: false,
        prefill_install_dir: None,
        prefill_all_users: false,
        prefill_desktop_shortcut: true,
        auto_install: false,
    };
    for arg in &args {
        if arg == "--uninstall" {
            out.uninstall = true;
        } else if arg == "--all-users" {
            out.prefill_all_users = true;
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
