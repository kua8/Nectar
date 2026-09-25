use std::path::Path;
use windows::Win32::System::Com::{CoCreateInstance, IPersistFile, CLSCTX_ALL};
use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};
use windows::core::{Interface, PCWSTR};

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn create_shortcut(
    lnk_path: &Path,
    target: &Path,
    args: &str,
    icon: &Path,
    description: &str,
) -> windows::core::Result<()> {
    unsafe {
        let shell_link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_ALL)?;

        let target_w = wide(&target.to_string_lossy());
        shell_link.SetPath(PCWSTR(target_w.as_ptr()))?;

        if !args.is_empty() {
            let args_w = wide(args);
            shell_link.SetArguments(PCWSTR(args_w.as_ptr()))?;
        }

        if let Some(dir) = target.parent() {
            let dir_w = wide(&dir.to_string_lossy());
            shell_link.SetWorkingDirectory(PCWSTR(dir_w.as_ptr()))?;
        }

        let icon_w = wide(&icon.to_string_lossy());
        shell_link.SetIconLocation(PCWSTR(icon_w.as_ptr()), 0)?;

        let desc_w = wide(description);
        shell_link.SetDescription(PCWSTR(desc_w.as_ptr()))?;

        let persist_file: IPersistFile = shell_link.cast()?;
        let lnk_w = wide(&lnk_path.to_string_lossy());
        persist_file.Save(PCWSTR(lnk_w.as_ptr()), true)?;
    }
    Ok(())
}
