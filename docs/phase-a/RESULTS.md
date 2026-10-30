# Phase A results

Phase A is the native foundation: workspace, shared core, event-driven media and lyrics, and a
runnable widget with DPI aware rendering and cached layers. The screenshots in this folder are
for review, not a design sign-off.

Machine: Windows 11 26300, 16 logical cores, single 100% monitor, NVIDIA RTX 3050. Release
build, `cargo build --release -p iceberg-widget`.

## What was built

- `app/core`: config schema/versioning/validation/atomic writes, title cleanup and track ids,
  SMTC watcher (`GlobalSystemMediaTransportControlsSessionManager`, event subscriptions, no
  polling), LRCLIB client with enhanced word timings and a 30 MB cache, font parsing and
  metric normalization, six built-in presets, IPC message schema/framing, single-instance
  guard.
- `app/widget`: winit + `UpdateLayeredWindow` + cosmic-text/swash. Per-monitor DPI v2 is set
  before the event loop. The surface is allocated in physical pixels.
- Modes: Card, Lyrics only, Island. Designed states: empty, loading, no lyrics, instrumental,
  unsynced, offline error.
- Static background layer cached separately from the lyric text layer. Shaped lyric lines are
  rasterized once and cached; a transition redraws only the text layer and composites it over
  the cached background.
- Lazy window: with no playback the process is tray-only and has no overlay window. The window
  is created on the first allowed SMTC track and destroyed when playback ends.
- Tray: show/hide and quit. The icon is the system application icon for now.
- `--demo`, `--screenshot=DIR`, `--bench=FILE`, `--scale=...`, `--quit-after=...` are product
  diagnostics used for this phase.

## Window style report

Measured from `GetWindowLongPtrW`, not assumed from winit defaults:

```
style=2516582400 ex_style=134742168
borderless=true tool_window=true no_activate=true topmost=true layered=true
app_window=false click_through=false visible=true foreground=false
```

`foreground=false` after creation and after the benchmark, so the overlay never takes focus.
There is no taskbar button because `WS_EX_TOOLWINDOW` is set and `WS_EX_APPWINDOW` is cleared.

## DPI

`dpi_awareness=per_monitor_v2` in all runs. The renderer was exercised at 100%, 150% and 200%
with the scale override because this machine has one 100% monitor. The live
`ScaleFactorChanged` and monitor-move handlers resize the surface and rebuild the cached layers
through the same path; a true mixed-DPI monitor test is still a Phase E item.

| Scale | Surface | Transition CPU (one core) | Transition CPU (total) | Idle CPU | Working set | Private |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 100% | 388x245 | 10.24% | 0.64% | 0% | 17.2 MB | 4.1 MB |
| 150% | 582x368 | 15.09% | 0.94% | 0% | 19.7 MB | 6.1 MB |
| 200% | 776x490 | 23.17% | 1.45% | 0% | 22.9 MB | 9.0 MB |

Transition measurement: 12 line changes, 240 ms apart, each with a 200 ms cached-text
transition, over a 2.9 s window. `background_rebuilds_during_transitions=0` in every run. The
idle figure is the 2 s after the transitions stop.

Tray-only (media filtered out, workspace trimmed once after startup): **3.3 MB working set,
2.6 MB private, 0% CPU**, no overlay window, `MainWindowHandle` points only at the hidden
tray message window. Release executable: **3,304,960 bytes (3.30 MB)**.

Raw benchmark reports: `bench-100.json`, `bench-150.json`, `bench-200.json`.

## What the screenshots show

- `card-100.png`, `card-150.png`, `card-200.png`: Card mode at the three scales, with the
  current demo lyric emphasized and the surrounding lines dimmed.
- `lyrics-100.png`: Lyrics-only mode, no cover or chrome.
- `island-100.png`: compact Island mode.
- `state-empty-100.png`, `state-loading-100.png`, `state-nolyrics-100.png`,
  `state-instrumental-100.png`, `state-unsynced-100.png`: the designed non-playing and
  failure states.
- `card-arabic-100.png`: the demo translated to Arabic to exercise RTL shaping and fallback.

The preview backdrop is a neutral review gradient; the real widget is transparent over the
desktop.

## Deliberately not in Phase A

- The Tauri settings app is not rebuilt yet. The old prototype in `app/src-tauri` remains for
  reference until Phase D replaces it; it is excluded from the new cargo workspace.
- The named-pipe transport is not wired. The message schema and line framing are in
  `core/src/ipc.rs`; settings will consume them in Phase D.
- Wallpaper, Acrylic, Ice and Liquid glass still render as flat fallbacks in the widget. The
  shared CPU backdrop pipeline is Phase C, as agreed after the renderer spike.
- Stock fonts, the font picker, per-song font UI, user font upload, and word-level highlight
  rendering are Phase B/D. The core font parser and metric normalization are ready and tested.
- Global hotkeys, fullscreen auto-hide, idle fade/destroy, edge snapping, and persisted drag
  positions are Phase E. Tray show/hide and quit are wired.
- The tray icon is the stock Windows icon, not the Iceberg icon.
- Multi-monitor and real mixed-DPI behavior still need a second-monitor test.
- The old README, CONTRIBUTING and release workflow still describe the Tauri prototype. They
  will be replaced when the settings app lands.
