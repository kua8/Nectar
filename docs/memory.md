# Memory and CPU footprint

Nectar is an always-on overlay: five windows, a WebView2 process tree and a
Rust host that polls Windows. That shape makes footprint the feature, not an
afterthought. This document records what was measured, what was changed and
what the trade-offs are.

Everything below is measured with the scripts in `scripts/` on a release build,
five monitors, default settings, nothing playing, hands off the mouse.

```
.\scripts\bench-ram.ps1 -Detail        # launch, settle, measure, quit
.\scripts\measure-ram.ps1 -Detail      # measure a running instance
```

## Result

| Metric (idle, Task Manager's view) | Before | After | Change |
|---|---|---|---|
| Private working set ("Memory") | 286.4 MB | **14.7 MB** | −95% |
| Working set (all processes) | 817 MB | **38.5 MB** | −95% |
| Commit (private bytes) | 489 MB | **217 MB** | −56% |
| Processes | 11 | 8 | −3 |
| Renderer processes | 5 | 2 | −3 |
| Threads | 361 | 204 | −157 |
| Idle CPU | 0.067 cores | **0.032 cores** | −52% |

With the settings window opened on demand: 9 processes, ~2 MB private working
set at rest, 279 MB commit.

Both figures move a lot during the seconds after a window opens (a new renderer
has to fault itself in), so samples are averaged and the method is in the
scripts, not in prose.

## What was actually using the memory

Per-process breakdown of the "before" case, private working set:

| Process | Private WS | Commit |
|---|---|---|
| GPU | 53.6 MB | 166.5 MB |
| Renderer × 5 (one per window) | 147.2 MB | 197.0 MB |
| Host (`nectar.exe`) | 37.2 MB | 42.8 MB |
| Browser | 31.5 MB | 40.5 MB |
| Network + storage utilities | 11.1 MB | 20.8 MB |
| Crashpad | 1.8 MB | 3.0 MB |

Three things dominate, and none of them is "the app is doing something":

1. **One renderer per window.** Tauri creates every window listed in
   `tauri.conf.json` at startup, and each WebView2 control gets its own renderer
   with its own document, style sheets, JS heap and compositor surface. The
   settings and calendar windows were paid for from launch even though most
   sessions never open them.
2. **Chromium's desktop defaults.** A 250 MB disk cache, a spare renderer kept
   warm for a navigation that never happens, background networking, component
   updates and a crash reporter.
3. **Pages that were paid for once and never handed back.** Windows keeps
   touched pages resident until something asks for them back. An overlay that
   has been idle for an hour still reports the footprint it had while it was
   busy, and Chromium is especially reluctant to give its own caches back.

## Changes

### Windows exist only when needed — `src-tauri/src/lazy_windows.rs`

Settings and calendar were removed from `tauri.conf.json` and are now built the
first time they are opened (`open_settings_window` / `open_calendar_window`).
Closing hides rather than destroys, so reopening is instant and unsaved edits
survive, and the hidden WebView is asked for
`COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW`, which is WebView2's documented way
to let an inactive view drop decode and paint caches without losing its DOM.

The notch, dock and overlay stay eager: they are the app, and two of them own
Win32 app bars.

### Fewer WebView2 processes doing desktop-browser work — `webview_tuning.rs`

Applied through `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` before the first window
exists, because WebView2 reads its arguments once when the environment is
created:

* the three wry defaults (`msWebOOUI`, `msPdfOOUI`, `msSmartScreenProtection`)
  are kept explicitly, because setting the variable replaces the default string;
* `SpareRendererForSitePerProcess` off — a spare renderer process kept warm for
  navigations that never happen;
* background networking, component updates, domain reliability, sync, the
  first-run flow and the crash reporter off;
* 16 MB disk cache and 8 MB media cache instead of desktop-sized defaults (the
  bundle is ~500 KB and artwork arrives from Rust as data URLs);
* `--enable-low-end-device-mode`, Chromium's own low-footprint profile for tile
  and decode caches;
* `--js-flags=--max-semi-space-size=1` — one isolate lives here;
* `--renderer-process-limit=1`.

`--disable-gpu` is deliberately **not** in the default set. Microsoft's WebView2
performance guidance is explicit that the GPU must stay enabled for rendering,
and this UI leans on `backdrop-filter` and `filter: blur` (14 and 21
declarations): software rasterising those costs more than the GPU process does.
It is behind the opt-in `nectar-ram-ultra` setting instead, where it is worth
~140 MB of commit.

### Idle pages are handed back — `src-tauri/src/mem.rs`

A single worker thread trims the whole process tree with `EmptyWorkingSet`
(the host through `GetCurrentProcess`, the WebView2 children through
`OpenProcess(PROCESS_SET_QUOTA)`, found by walking a toolhelp parent map so the
renderers under the browser process are included).

The cadence is the whole trick:

* nothing happens until the app has been idle for 3 s, where idle is the smaller
  of "no keyboard or mouse input anywhere" (`GetLastInputInfo`) and "nothing has
  touched one of our windows";
* never while a menu is open, the dock is being dragged, or the splash is
  playing;
* re-trim every 3 s while the idle stretch lasts. Measured: with a 10 s cadence
  the tree climbed back to ~125 MB between trims and averaged 92 MB; at 3 s it
  averages ~45 MB even while the notch's clock keeps repainting;
* media playback uses a slower 15 s cadence — the visualiser repaints
  continuously, so an aggressive trim would mostly fault pages it is about to
  touch again.

Trimming hands the same pages straight back on the next touch as soft faults
(microseconds, no I/O), which is why a 3 s cadence is cheap. A hidden settings
window is trimmed like everything else.

Both knobs are user-visible: **Memory Saver** (`nectar-ram-saver`, default on,
applied live) and **Maximum Memory Saving** (`nectar-ram-ultra`, default off,
needs a restart because WebView2 arguments are read once).

### The host stops polling — `services.rs`

The system worker used to run a fixed 64 ms loop forever: two COM calls to the
audio endpoint 15 times a second whether or not anything had changed, and a
`try_recv` for commands. It now **blocks on the command channel** with a timeout
(`recv_timeout`), so a media key is handled the instant it arrives and an idle
app wakes 5 times a second instead of 15, then drops to the fast cadence for two
seconds after any activity so a volume slider still feels immediate.

This is the single biggest CPU change in the app: the host went from 0.041 to
0.026 cores on its own.

### React: stop doing work nobody can see

* **Visibility-gated polling** — `src/hooks/visibleInterval.ts` wraps
  `setInterval` so callbacks do not run while the window is hidden. This matters
  for the overlay, settings and calendar windows (which are genuinely hidden); it
  is a no-op for the notch and dock, which stay visible and merely translate the
  pill offscreen.
* **One IPC instead of four** — the status widgets ask for CPU, RAM, disk and
  network together through `get_system_metrics`; previously that was four
  invokes every 3 s, four JSON payloads and four wake-ups for two small numbers.
* **Battery-saver poll** 5 s → 15 s (the value changes on a minute scale), and
  the Wi-Fi poll only runs while the command centre is open.
* **Startup polls back off** — the notch and dock used to ask the host whether
  they were visible 10 and 5 times a second *forever* if they never became
  visible (a dock switched off, a notch hidden in smart mode). After five
  seconds they back off to ~1 Hz and keep waiting.
* **Removed a discarded app scan** — the dock called `get_installed_apps` on
  every launch and threw the result away: a full Start Menu scan on a cold
  cache, then a multi-hundred-KB payload across IPC, during the splash.
* **Audio visualiser**: the capture loop produced ~90 FFT frames a second and
  broadcast every one to all five windows. It now publishes at ≤30 Hz and only
  to notch windows, which is the only place the bars exist.
* **No re-sent artwork** — `media-update` fires whenever the playback position
  drifts, which is every poll while a track plays; it was carrying the whole
  base64 cover image each time. Artwork is now sent only when it changes and the
  frontend keeps the image it already has.
* **Animation and render waste** (from a frontend audit): removed a permanently
  applied inert `filter: blur(0px)` (which forces a filter surface and a stacking
  context on the element containing the whole UI) and a redundant
  `pointerEvents` from the notch's animated object; hoisted the dock's
  `variants` object out of the component body so a hover no longer gives every
  icon a new identity; removed two 700 ms `requestAnimationFrame` "chases" that
  re-measured a box the `ResizeObserver` had already measured in the same frame;
  and made the dock keep its previous array identity when the window list has not
  changed, so a 10 s poll no longer tears down and restarts its measurement
  effect.

## Building it correctly

`cargo build --release` alone produces a **dev-mode** binary: Tauri decides
`dev` from the `custom-protocol` feature, and in dev mode every window points at
`http://localhost:1420` and shows `ERR_CONNECTION_REFUSED` when Vite is not
running. Build the production binary with either

```
bun run tauri build                                   # normal path
cargo build --release --features tauri/custom-protocol  # when driving cargo
```

## Reproducing the numbers

* `scripts/nectar-mem.ps1` — collects the process tree (parent-PID walk, so
  other apps' WebView2 processes are never counted) and reports private working
  set, commit and working set per process.
* `scripts/measure-ram.ps1` — averaged samples for a running instance.
* `scripts/bench-ram.ps1` — launches, settles, samples, then shuts down through
  the app's own close path so the taskbar is restored (with a direct fallback if
  the process has to be killed).

## References

* [Performance best practices for WebView2 apps](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/performance)
  — the source for the memory-usage target level, keeping the GPU enabled,
  batching host traffic, and preferring CSS animations.
* [`content-visibility` is now Baseline](https://web.dev/blog/css-content-visibility-baseline)
  — used as the reference for the virtualisation advice, though no list here is
  long enough to need it yet.
* The patterns that transfer from other native work: an idle app should block on data rather than poll, a reclaim must be cadence-aware and skip while animating, and caches need hard caps rather than good intentions.
