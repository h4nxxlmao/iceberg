# Iceberg

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

A tiny, transparent now-playing and synced-lyrics widget for Windows. It reads the current song from Windows' own media session, fetches synced lyrics from LRCLIB, and stays out of the way.

No Spotify login. No API keys. No account. No telemetry. No WebView in the widget.

> **Rebuild status.** The native widget lives in `app/core` and `app/widget`. The settings app, installer, hotkeys, and remaining desktop polish are still in progress. The Tauri/WebView2 app in `app/src-tauri` is the previous prototype and is not the current build.
>
> Renderer decision: [docs/decisions/0001-renderer.md](docs/decisions/0001-renderer.md). Native measurements and review screenshots: [Phase A](docs/phase-a/RESULTS.md) and [Phase B](docs/phase-b/RESULTS.md).

![Native Card preview](docs/phase-b/card-100.png)

Offscreen render over a neutral review backdrop. The real widget is a transparent layered window on the desktop.

## Current native widget

- Frameless, always-on-top widget with Island, Card, and Lyrics-only modes
- Event-driven Windows SMTC now-playing; no polling loop
- Time-synced lyrics from [LRCLIB](https://lrclib.net), with plain-text, instrumental, loading, empty, unsynced, and offline states
- Cached static background and shaped lyric masks; a lyric transition redraws only the text layer
- Slide, fade, and off transitions, plus the configured active color, inactive opacity, alignment, case, letter spacing, line height, shadow, and outline
- Eight bundled OFL serif, sans, mono, handwritten, condensed, rounded, and display families, loaded lazily and normalized for optical size and baseline. Font picker and upload UI come with the settings app.
- Per-song lyric offsets and per-song font overrides in the config schema
- Tray show/hide/quit, drag-to-move, click-through option, demo mode, and offscreen screenshot/benchmark diagnostics
- Exact tool-window styles set and verified in code: borderless, no activate, topmost, layered, no taskbar button, never foreground
- Tray-only mode creates no window and loads no font data until playback exists

The settings app is a separate process that will be created on demand and exit completely when closed. It is not built yet.

## Build from source

Requirements: Windows 10/11 x64 and a stable Rust toolchain.

```powershell
cd app
cargo build --release -p iceberg-widget
cargo test --workspace
```

Run the widget with the built-in demo instead of a media session:

```powershell
cargo run -p iceberg-widget -- --demo
```

Render review screenshots and exit:

```powershell
cargo run -p iceberg-widget -- --screenshot=../docs/phase-b
```

Run the cached-text transition benchmark:

```powershell
cargo run -p iceberg-widget -- --bench=../docs/phase-b/bench-100.json --scale=100
```

The release executable is `app/target/release/iceberg.exe`. For a packaged build, `app/assets/fonts` is copied to a `fonts` folder next to the executable. The installer and settings app are not available yet; do not use the old `npm run tauri build` path for the native widget.

## Measurements

Windows 11, release build, default Lato font, 12 lyric transitions at 200 ms with 240 ms spacing. CPU is process CPU time over a 2.9 s transition window plus a 2 s idle window. Three runs per scale; transition CPU shows the median, idle shows the observed range. Total CPU is normalized across the build machine's 16 logical processors. Working set and private bytes are for `iceberg.exe`; there are no child processes.

| Scale | Surface | Transition CPU (one core / total) | Idle CPU | Working set / private |
| ---: | ---: | ---: | ---: | ---: |
| 100% | 388 x 245 | 5.39% / 0.34% | 0% | 20.6 / 6.9 MB |
| 150% | 582 x 368 | 8.08% / 0.51% | 0% | 23.2 / 9.1 MB |
| 200% | 776 x 490 | 12.93% / 0.81% | 0–0.78% / 0–0.05% | 26.8 / 12.0 MB |

Transition one-core ranges across the three runs: 4.85–5.93%, 7.54–8.62%, and 12.93–14.55%. At 200%, two of three idle samples read 0.78% of one core (0.05% total) and one read 0.

Background rebuilds during transitions: **0**. The 150% and 200% rows use `--scale` because the build machine has a single 100% monitor; real mixed-DPI monitor moves are still untested.

The widget stays under the 40 MB software-rendering budget at every tested scale. The release executable is **3.20 MiB**; the bundled font set is **2.85 MiB**. The font library only reads the active family: a tray-only session carries no font data. The last clean tray-only measurement was 3.3 MB working set / 2.6 MB private; a fresh tray-only sample on the build machine was not possible because a live media session kept the widget window open.

The font previews use the same card at every family: 388 x 245 at 100%. Optical size and baseline are normalized from each font's cap height and ascender; line spacing stays on the user's line-height grid so the widget does not resize when the family changes.

## Typography

The stock families are Lato, Poppins, Abril Fatface, Barlow Condensed, Amatic SC, Space Mono, Varela Round, and Lobster. Each is SIL Open Font License 1.1; the license texts and sources are in [app/assets/fonts](app/assets/fonts/README.md).

User fonts are read from `%LOCALAPPDATA%\Iceberg\fonts`. The core validates the file, rejects empty or oversized files, reads family/subfamily/metrics/variable axes, and falls back to Lato if a requested family cannot be resolved. Weight uses the family's bold face when available and a synthetic embolden above 600 when it is not; italic is a synthetic shear so the control remains real for families without an italic file.

Word timings from enhanced LRC are parsed by the core but are not highlighted yet.

## Configuration and data

| Path | Contents |
| --- | --- |
| `%APPDATA%\Iceberg\config.json` | Settings, per-song offsets and font overrides, presets |
| `%LOCALAPPDATA%\Iceberg\cache` | Lyrics and artwork cache |
| `%LOCALAPPDATA%\Iceberg\fonts` | User-imported font files |
| `%LOCALAPPDATA%\Iceberg\presets` | User presets |

The config is versioned, migrated, and validated on load. Missing or invalid values fall back to defaults instead of crashing. Older configs that referenced system fonts are migrated to the closest stock family.

## Privacy and security

- The only network requests Iceberg makes go to `lrclib.net` over HTTPS, with timeouts and response size limits.
- Lyrics, titles, artists, and artwork are untrusted data. The native renderer draws them as text; there is no HTML or WebView in the widget.
- No telemetry, analytics, accounts, or crash reporting.
- Font files and config are validated before use. The widget's Tauri capabilities will be least-privilege when the settings app lands.
- No remote content is loaded in the settings app.

See [SECURITY.md](SECURITY.md) for reporting.

## Not done yet

- Settings app, named-pipe IPC, presets UI, font picker and font upload UI
- Installer and resource packaging
- Global hotkeys, per-track offset hotkeys, fullscreen auto-hide, idle auto-hide, edge snapping, lock, and real multi-monitor/DPI moves
- Windows 10 verification, two-monitor testing, and the 20-minute leak run
- Word-level lyric highlighting and the live-capture backdrop experiment

The old Tauri prototype under `app/src-tauri` is kept for reference only. The current widget does not use WebView2.

## Project layout

```
app/
  core/          shared config, SMTC, LRCLIB, cache, fonts, presets, IPC schema
  widget/        native always-on widget, renderer, tray, diagnostics
  assets/fonts/  bundled OFL font files and licenses
  src-tauri/     previous Tauri prototype (reference only)
docs/            design notes, renderer decision, phase screenshots and measurements
```

## Contributing

Read [CONTRIBUTING.md](CONTRIBUTING.md). Run `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` from `app` before opening a pull request.

## License

MIT. See [LICENSE](LICENSE).
