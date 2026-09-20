# Nectar Settings Reference

All settings are stored in `settings.json` in the app config directory (`%APPDATA%/com.kua8.nectar/`, derived from the Tauri app identifier). The file is a flat JSON object with `nectar-` prefixed keys. Nectar watches this file for external changes and applies them in real-time.

## Quick Start

Edit `settings.json` with any text editor while Nectar is running. Changes are applied immediately — no restart required.

```json
{
  "nectar-dock-enabled": "true",
  "nectar-dock-mode": "smart",
  "nectar-theme-mode": "dark",
  "nectar-scale": "1.0"
}
```

## Settings Keys

### Dock

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `nectar-dock-enabled` | `"true"` / `"false"` | `"true"` | Show or hide the Nectar Dock (taskbar replacement). |
| `nectar-dock-mode` | `"fixed"` / `"smart"` / `"peek"` | `"fixed"` | Dock visibility behavior. **fixed** = always visible as AppBar. **smart** = auto-hide when overlapped by another window or a fullscreen app. **peek** = hidden until cursor approaches bottom edge. Smart/peek still reveal on hover and are clickable even over a fullscreen app — only a thin pre-emptive strip right at the screen edge stays click-through, so other apps' own edge-hugging UI (e.g. a Snipping Tool selection) is never intercepted. |
| `nectar-dock-preview-enabled` | `"true"` / `"false"` | `"true"` | Show window thumbnail previews when hovering dock icons. |
| `nectar-dock-search-enabled` | `"true"` / `"false"` | `"true"` | Show a search icon next to Start that opens Windows Search (Win+S). |
| `nectar-dock-icon-only` | `"true"` / `"false"` | `"false"` | Minimal icon-only style (no background/padding around icons). |
| `nectar-dock-mixed-reorder` | `"true"` / `"false"` | `"false"` | `"false"` = pinned and running icons stay in two separate groups with a divider between them. `"true"` = a single group — any icon, pinned or not, can be dragged anywhere. |

### Notch

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `nectar-notch-mode` | `"fixed"` / `"smart"` / `"peek"` | `"fixed"` | Notch (top bar) visibility behavior. **fixed** = always visible, never auto-hides (including over fullscreen apps — matches a real hardware notch, which doesn't disappear either). **smart** = hides when overlapped by another window/fullscreen app, reveals on hover. **peek** = hidden until cursor approaches the top edge, a media event, or a notification. Smart/peek reveal and are fully clickable on hover even over a fullscreen app (only a thin strip right at the screen edge stays click-through, so it never steals input meant for another app). Unlike the Dock, the notch never reserves desktop work-area space, so it never leaves a gap above maximized windows' content. |

### Weather

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `nectar-weather-enabled` | `"true"` / `"false"` | `"true"` | Show weather widget in the notch status bar. |
| `nectar-weather-city` | string | `""` | Manually set city name for weather. Empty string = auto-detect via IP geolocation. |
| `nectar-weather-lat` | number string | (auto) | Latitude coordinate for weather. Set automatically when a city is selected. |
| `nectar-weather-lon` | number string | (auto) | Longitude coordinate for weather. Set automatically when a city is selected. |
| `nectar-weather-cached-temp` | number string | (none) | Cached temperature value shown before next API fetch. |
| `nectar-weather-cached-condition` | string | (none) | Cached weather condition text (e.g. "Partly Cloudy"). |
| `nectar-temp-unit` | `"celsius"` / `"fahrenheit"` | `"celsius"` | Temperature display unit. |

### Modules

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `nectar-calendar-enabled` | `"true"` / `"false"` | `"true"` | Enable calendar/timer mode in the notch. |
| `nectar-music-mode-enabled` | `"true"` / `"false"` | `"true"` | Enable interactive music media widget. |
| `nectar-music-compact-notch` | `"true"` / `"false"` | `"true"` | Show compact music display (visualizer + artwork) in collapsed notch. |

### Music Appearance

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `nectar-media-layout` | `"classic"` / `"compact"` | `"classic"` | Expanded player style. **classic** = large album art. **compact** = small thumbnail + controls. |
| `nectar-media-ambience-enabled` | `"true"` / `"false"` | `"true"` | Colored ambient glow behind expanded album art. |
| `nectar-media-compact-glow-enabled` | `"true"` / `"false"` | `"true"` | Glow effect around the collapsed compact thumbnail. |
| `nectar-media-visualizer-enabled` | `"true"` / `"false"` | `"true"` | Audio visualizer bars in music mode. Also accepts `nectar-visualizer-enabled` (legacy alias). |
| `nectar-media-album-art-enabled` | `"true"` / `"false"` | `"true"` | Show album artwork in the notch music display. |

### Overlays

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `nectar-volume-overlay-enabled` | `"true"` / `"false"` | `"true"` | Show Nectar volume HUD when volume changes (replaces native Windows OSD). |
| `nectar-volume-edge-enabled` | `"true"` / `"false"` | `"true"` | Trigger volume HUD by hovering the left screen edge. |
| `nectar-brightness-overlay-enabled` | `"true"` / `"false"` | `"true"` | Show Nectar brightness HUD when brightness changes. |
| `nectar-brightness-edge-enabled` | `"true"` / `"false"` | `"true"` | Trigger brightness HUD by hovering the right screen edge. |

### Appearance

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `nectar-theme-mode` | `"dark"` / `"light"` / `"custom"` / `"adaptive"` | `"dark"` | Theme mode. **dark** = dark translucent. **light** = light translucent. **custom** = user-picked color. **adaptive** = follows Windows system accent color. |
| `nectar-theme-color` | hex string | `"#007aff"` | Custom theme color (used in `custom` and `adaptive` modes). |
| `nectar-theme-opacity` | float string | `"0.80"` | Background opacity (0.1 to 1.0). |
| `nectar-theme-saturation` | float string | `"0.50"` | Color saturation for custom/adaptive themes (0.0 to 1.0). |
| `nectar-theme-brightness` | float string | `"0.15"` | Background brightness for custom/adaptive themes (0.0 to 1.0). |
| `nectar-corners-enabled` | `"true"` / `"false"` | `"false"` | Render rounded screen corner overlays on top edges. |

### Status Widgets

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `nectar-status-widgets` | JSON string | `{"left":["weather"],"right":["battery"]}` | Widget layout in collapsed notch. Available: `"weather"`, `"battery"`, `"cpu"`, `"ram"`, `"disk"`, `"net"`. The `"battery"` widget (and its low-battery/charging pulses) is automatically suppressed on desktops with no physical battery — detected via Windows' `PowerManager.BatteryStatus`, not the browser's Battery API (which fakes a permanent "100%, charging" reading on desktops). It's also silently dropped from a placed config on such machines, even if set manually or imported from a laptop's settings file. |

Example:
```json
{
  "nectar-status-widgets": "{\"left\":[\"cpu\",\"ram\"],\"right\":[\"battery\",\"net\"]}"
}
```

### System

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `nectar-scale` | float string | `"1.0"` | UI scale factor (0.8 to 1.3). Changing this re-registers AppBars to resize the reserved screen area. |
| `nectar-low-battery-threshold` | integer string | `"20"` | Battery percentage that triggers the low-battery alert pulse (5 to 50, step 5). |
| `nectar-auto-update` | `"true"` / `"false"` | `"false"` | Check for and download updates automatically on startup. |
| `nectar-show-update-indicator` | `"true"` / `"false"` | `"true"` | Show a green dot on the notch when an update is available. |
| `nectar-time-format-24h` | `"true"` / `"false"` | `"false"` | Use 24-hour clock format in the notch. When `"false"`, displays 12-hour format with AM/PM. |

### Internal (Do Not Edit Manually)

| Key | Type | Description |
|-----|------|-------------|
| `nectar-first-run` | sentinel | Set to `"done"` after first launch. Triggers splash screen if absent. |
| `nectar-app-version` | string | Last known app version. If it differs from current, splash screen is shown on update. |

## Event System

Nectar uses two Tauri events for settings synchronization:

### `settings-changed`
- **Emitted by:** `save_setting` command (frontend or backend)
- **Payload:** `{ "key": "nectar-...", "value": ... }`
- **Purpose:** Broadcasts changes made through the Nectar UI to all windows
- **Key format:** Nectar-prefixed keys as-is (e.g. `"nectar-dock-mode"`)

### `settings-external-changed`
- **Emitted by:** File watcher (detects external edits to `settings.json`)
- **Payload:** `{ "key": "nectar-...", "value": ... }` or `{ "key": "nectar-...", "value": null }` for removed keys
- **Purpose:** Broadcasts changes made by external editors (VS Code, notepad, scripts)
- **Key format:** Nectar-prefixed keys as-is
- **Behavior:** Also syncs values to `localStorage` for instant frontend reads

### Flow
```
External editor saves settings.json
    ↓
File watcher detects change (ReadDirectoryChangesW)
    ↓
Diffs against SETTINGS_CACHE
    ↓
Emits settings-external-changed for each changed/removed key
    ↓
useSettingsSync hook updates React state + localStorage
    ↓
UI re-renders with new values
```

## Example: Changing Dock Mode via Script

```powershell
# PowerShell: switch dock to smart mode
$json = Get-Content "$env:APPDATA\com.kua8.nectar\settings.json" | ConvertFrom-Json
$json.'nectar-dock-mode' = 'smart'
$json | ConvertTo-Json | Set-Content "$env:APPDATA\com.kua8.nectar\settings.json"
```

```python
# Python: disable dock
import json, os
path = os.path.join(os.environ['APPDATA'], 'com.kua8.nectar', 'settings.json')
with open(path) as f: settings = json.load(f)
settings['nectar-dock-enabled'] = 'false'
with open(path, 'w') as f: json.dump(settings, f)
```

## Notes

- All boolean values are strings (`"true"` / `"false"`) for consistency with `localStorage`.
- The `useSettingsSync` hook auto-converts `"true"` / `"false"` strings to booleans.
- `auto-hide` mode values in `nectar-dock-mode` and `nectar-notch-mode` are legacy aliases for `smart` — they are mapped automatically.
- Changing `nectar-scale` triggers AppBar re-registration to adjust reserved screen space.
- Theme changes (`nectar-theme-*`) are applied by reading all theme values from `localStorage` and calling `applyTheme()` — the theme system depends on all five theme keys being in sync.
