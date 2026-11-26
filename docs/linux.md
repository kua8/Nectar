# Nectar on Linux

Nectar runs on Linux desktops that use X11: Cinnamon (Linux Mint), MATE, Xfce, and GNOME or KDE on Xorg. On a Wayland session it starts through XWayland, but window tracking, focus and click-through only work for X11 apps there, so an Xorg session is recommended.

## Install

From a release, install the `.deb` (Debian, Ubuntu, Mint):

```sh
sudo apt install ./nectar_1.11.0_amd64.deb
```

Or use the AppImage: `chmod +x Nectar*.AppImage && ./Nectar*.AppImage`.

Nectar uses these system tools when they are present:

| Tool | Used for | Package |
| --- | --- | --- |
| `playerctl` | Now playing, media controls | `playerctl` |
| `wpctl` or `pactl` | Volume, output device, per-app volume | `wireplumber` / `pulseaudio-utils` |
| `nmcli` | Wi-Fi and Ethernet | `network-manager` |
| `bluetoothctl` | Bluetooth | `bluez` |
| `busctl` (logind) | Screen brightness without root | `systemd` |
| `gio`, `xdg-open` | Launching apps and files | `libglib2.0-bin`, `xdg-utils` |

## Build from source

```sh
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev \
  librsvg2-dev libxdo-dev libssl-dev libdbus-1-dev build-essential pkg-config
curl -fsSL https://bun.sh/install | bash
curl -sSf https://sh.rustup.rs | sh

bun install
bun run tauri dev                    # development
bun run tauri build --bundles deb    # package
```

## How it maps from Windows

| Windows | Linux |
| --- | --- |
| Hides the taskbar | On Cinnamon, the panel is set to auto-hide with a very long reveal delay while the dock is on, so the screen edge opens the Nectar dock. The Super key menu still works. Your panel settings come back when Nectar quits, is killed, or crashes (restored on the next start). Turn this off with `"nectar-linux-hide-panel": "false"` in `~/.config/com.kua8.nectar/settings.json`. |
| Appbar ("Fixed" dock) | `_NET_WM_STRUT_PARTIAL`, so maximized windows stop above the dock |
| Win32 window list | EWMH (`_NET_CLIENT_LIST`, `_NET_ACTIVE_WINDOW`) |
| Start button / Search | Opens the desktop's app menu (Super key) |
| Task View | `Ctrl+Alt+Down` (Cinnamon/Muffin window overview) |
| Start Menu shortcuts and exe paths | `.desktop` entries, icons from the current icon theme |
| SMTC media sessions | MPRIS through `playerctl` |
| Core Audio | PipeWire / PulseAudio |
| WMI brightness | `/sys/class/backlight` and logind `SetBrightness` |
| Credential Manager (CalDAV password) | Secret Service (GNOME Keyring / KWallet) |
| Autostart | `~/.config/autostart` |

Settings live in `~/.config/com.kua8.nectar/`, the log in `~/.local/share/com.kua8.nectar/logs/nectar.log`.

## Not on Linux yet

- Live (DWM) window previews: the dock shows captured snapshots instead, and minimized windows have no preview.
- Notification area (system tray) icons in the dock, the agent API and the global notch hotkey.
- Wallpaper glass backdrop and syncing the accent color to the system.
- Self-update: update through your package manager or a new AppImage.

## Troubleshooting

- **Blank or black windows** on NVIDIA or hybrid graphics: Nectar sets `WEBKIT_DISABLE_DMABUF_RENDERER=1` by default. If it is still blank, try `WEBKIT_DISABLE_COMPOSITING_MODE=1 nectar`.
- **No transparency**: a compositor is needed. Cinnamon, GNOME, KDE and Xfce (with compositing enabled in Window Manager Tweaks) have one.
- **The dock hides behind the panel** on desktops other than Cinnamon: set your panel to auto-hide, or move it to the top.
- **Hit-test debugging**: start with `NECTAR_HIT_DEBUG=1 nectar` to log every click-through change.
