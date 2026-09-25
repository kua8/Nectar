<div align="center">

<img src="docs/logo.png" width="160" alt="Nectar logo" />

<br />

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/wordmark-dark.png" />
  <img src="docs/wordmark-light.png" height="48" alt="Nectar" />
</picture>

**A notch and a dock for Windows: physics-animated, reactive, and built to replace your taskbar.**

<br />

[![Latest release](https://img.shields.io/github/v/release/kua8/Nectar?style=flat-square&color=2f81f7)](https://github.com/kua8/Nectar/releases/latest)
[![Downloads](https://img.shields.io/github/downloads/kua8/Nectar/total?style=flat-square&color=2f81f7)](https://github.com/kua8/Nectar/releases)
[![License](https://img.shields.io/github/license/kua8/Nectar?style=flat-square&color=2f81f7)](LICENSE)
![Platform](https://img.shields.io/badge/platform-Windows-2f81f7?style=flat-square)

[**Download**](https://github.com/kua8/Nectar/releases/latest) &nbsp;·&nbsp; [Features](#features) &nbsp;·&nbsp; [Multiple monitors](#multiple-monitors) &nbsp;·&nbsp; [Settings](SETTINGS.md) &nbsp;·&nbsp; [Build from source](#build-from-source)

<br />

<img src="docs/screenshots/desktop.webp" width="820" alt="Nectar's notch and dock on a Windows desktop" />

</div>

<br />

Nectar is a fork of [Bloom](https://github.com/SehajveerSingh2005/bloom) by Sehajveer Singh, and the concept and the codebase it grew from are theirs. It is rebranded end to end and maintained independently, with its own fixes and direction.

## Features

- **A notch that does things.** Music, quick controls, system stats, and a calendar in one spring-animated island at the top of your screen.
- **A dock that replaces your taskbar.** It mirrors your real taskbar (same pinned apps, same order, real icons and names) and hides the native one.
- **Native where it counts.** The system tray opens the actual Windows flyout, and Quit ends an app the way Task Manager's End task does.
- **Yours to shape.** Themes, three visibility modes, per-monitor placement, and every option in a plain `settings.json` you can edit live.
- **Light on the system.** Audio capture, cursor tracking, and thumbnail work pause themselves when nothing needs them.

## The Notch

A notch at the top of your screen that adapts to what you're doing. Scroll or swipe to change modes, or pin it in place like a hardware notch.

<div align="center">
<img src="docs/screenshots/island.png" width="420" alt="The Nectar notch showing the time and now-playing artwork" />
</div>

<br />

| Mode | What it does |
| --- | --- |
| **Music** | Album art, track info, playback controls, and a visualizer that reacts to five frequency bands. |
| **Command Center** | Wi-Fi, Bluetooth, Do Not Disturb, volume, and brightness without opening Windows Settings. |
| **Status** | Battery (hidden automatically on desktops without one) and weather, in a layout you choose. |
| **Calendar** | A month view with a built-in Pomodoro timer. |

Width, height, corner radius, and position each animate on their own spring, so every transition feels physical.

**Visibility modes** (Settings › Notch):

| Mode | Behavior |
| --- | --- |
| **Fixed** | Always visible, even over fullscreen apps, like a real hardware notch. |
| **Smart** | Hides when another window overlaps it and reveals on hover. |
| **Peek** | Hidden until the cursor reaches the top edge, or a media event or notification arrives. |

In Smart and Peek the notch stays fully clickable when it's revealed. Only a thin strip at the very edge of the screen stays click-through, so it never swallows input meant for a screenshot tool or a game. The notch never reserves desktop space, so it can't leave a gap above maximized windows.

## The Dock

A taskbar that moves. Drag icons to reorder, hover for window previews, and right-click for a context menu that always opens above the dock.

<div align="center">
<img src="docs/screenshots/dock.png" width="520" alt="The Nectar dock with pinned apps" />
</div>

<br />

- **Your taskbar, mirrored.** On first launch Nectar imports the apps pinned to your Windows taskbar, in the same order, including File Explorer, Discord, and Microsoft Store apps.
- **Real icons and names.** Windows Settings shows its gear and Task Manager is called "Task Manager", not `taskmgr`.
- **Pinned and running, separated.** A divider marks the split by default. Turn on **Mix Pinned & Running** to drag any icon anywhere.
- **Quit means quit.** Right-click › Quit ends the app like End task in Task Manager.
- **Search built in.** A search icon beside Start opens Windows Search.
- **Native tray.** The tray button opens the real Windows hidden-icons flyout under the notch.

Dock visibility uses the same **Fixed**, **Smart**, and **Peek** modes as the notch. Fixed reserves screen space like a normal taskbar, Smart hides when a window overlaps it, and Peek stays out of the way until you approach the bottom edge.

## Multiple monitors

The dock and the notch each have a **Show On** setting, chosen separately in Settings:

| Show On | Result |
| --- | --- |
| **Primary Monitor** | Shown on your primary display only. This is the default. |
| **All Monitors** | Every connected display gets its own copy. |
| **Specific Monitor** | Pinned to one display you choose from a list. |

Because the two are independent, the same app can look very different from one machine to the next:

- **Dock and notch on every monitor.** Each screen looks the same, and each dock reserves its own space at the bottom.
- **Dock on the primary only.** The other screens have no taskbar at all, since the Windows taskbar is hidden on every monitor while Nectar's dock is on. Maximized windows there use the full screen.
- **Notch on one screen, dock on another.** Works fine, and each one follows its own visibility mode.
- **Dock turned off.** The native Windows taskbar comes back on every monitor.

Extra monitors get their own windows, so **All Monitors** uses somewhat more CPU and memory than a single display. If you're on a low-power machine, keep **Show On** on one monitor.

## Under the hood

The backend is Rust and talks straight to the Windows shell: global input hooks, WASAPI for real-time system audio, COM for media session control, and WMI for hardware monitoring. The interface is React, animated with springs.

Every setting lives in one plain `settings.json` that Nectar watches and applies as you save, so you can script it or keep it in your dotfiles. [SETTINGS.md](SETTINGS.md) has the full reference. **Settings › About › Reset to Defaults** resets everything.

## Install

1. Download the installer from the [latest release](https://github.com/kua8/Nectar/releases/latest).
2. Choose **Just me** or **All users**. An all-users install asks for administrator permission once.
3. Nectar starts with a short splash animation, then the notch and dock appear together.

Nectar updates itself: a green dot on the notch means an update is ready, and you can turn on **Auto Update** in **Settings › About**. **Launch at Login** is in **Settings › General**.

**Uninstall** from Windows Settings › Apps, or from **Settings › About** inside Nectar. Both remove the app and put your native taskbar back.

If Nectar is ever force-closed, it restores the Windows taskbar the next time it starts.

## Build from source

You'll need [Rust](https://rustup.rs/) and [Bun](https://bun.sh/).

```bash
git clone https://github.com/kua8/Nectar.git
cd Nectar
bun install
bun run tauri dev
```

To produce a release build, run `bun run tauri build`.

## Contributing

Found a bug or have an idea? [Open an issue](https://github.com/kua8/Nectar/issues) or send a pull request. Say which Windows version and monitor setup you're on if it's a display bug, since that is often what decides it.

## License

Nectar is open source under the [GPLv3](LICENSE).
