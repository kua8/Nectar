# Nectar installer

A second, self-contained Tauri v2 app — not NSIS. This is what users actually
download and run to install Nectar. It's a real webview, so the UI is plain
HTML/CSS matching the app's own dark theme, not NSIS's native-dialog wizard.

## Why a separate app, and why NSIS is still around too

`tauri-plugin-updater` (used by the main app for auto-update) only recognizes
NSIS- or MSI-produced artifacts — it sniffs the downloaded file and, for an
`.exe`, unconditionally appends NSIS-specific silent-install flags
(`/UPDATE /ARGS ... /P /R` or `/S /R`). A custom installer exe can't be fed to
it. So:

- `src-tauri/`'s existing NSIS bundle config (`tauri.conf.json` →
  `bundle.windows.nsis`, `installer/`) is **untouched** and keeps producing the
  signed update artifact + `latest.json` for the auto-updater. Users never see
  this one — the updater runs it silently in the background.
- `installer-app/` (this project) is the pretty, custom-UI installer people
  download for a first install. It is not wired into the update pipeline at
  all.

## How it works

The main app's release exe is zipped and embedded into this installer's own
binary via `include_bytes!` (`src-tauri/src/payload.rs`). At runtime the
installer extracts that payload to the chosen install directory, creates
Start Menu / Desktop shortcuts (`src-tauri/src/shortcuts.rs`, via the same
`IShellLinkW` COM interfaces the main app already uses read-only for taskbar
pin import), and writes an `Uninstall` registry entry
(`src-tauri/src/registry.rs`) so it shows up in Add/Remove Programs.

Uninstalling reuses the same binary: the installer copies itself into the
install directory as `uninstall.exe` during install, and registers that path
with a `--uninstall` flag as the `UninstallString`. Launched with
`--uninstall`, it runs an uninstall UI instead of the install flow.

"Install for all users" needs admin rights. If the installer isn't already
elevated when that's chosen, it relaunches itself with the `runas` verb
(`src-tauri/src/elevate.rs`) carrying the user's choices as CLI args
(`--install-dir=`, `--all-users`, `--auto-install`, `--no-desktop-shortcut`),
so the elevated relaunch resumes straight into installing instead of making
the user redo the wizard after the UAC prompt.

## Building

1. **Build the payload** (from the repo root, PowerShell):

   ```powershell
   ./installer-app/scripts/build-payload.ps1
   ```

   This runs `tauri build --no-bundle` on the main app and zips
   `src-tauri/target/release/nectar.exe` into
   `installer-app/src-tauri/payload.zip`. Re-run it whenever the main app
   changes — `payload.zip` is not checked in (it's build output).

2. **Build the installer**:

   ```powershell
   cd installer-app
   npm install
   npm run tauri build -- --no-bundle
   ```

   `bundle.active` is `false` in `installer-app/src-tauri/tauri.conf.json`, so
   this just compiles a normal native exe — no NSIS/MSI wrapping. The output
   at `installer-app/src-tauri/target/release/nectar-installer.exe` **is**
   the shippable `Nectar-Setup.exe`; rename/upload it as-is.

## Known limitations

- No rollback if an install is interrupted partway through.
