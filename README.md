# Iceberg

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

A tiny, transparent now-playing and synced-lyrics widget for Windows. It reads the current song from Windows' own media session, fetches synced lyrics from LRCLIB, and stays out of the way.

No Spotify login. No API keys. No account. No telemetry. No WebView in the widget.

> **Rebuild status.** The native widget lives in `app/core` and `app/widget`. The settings app, installer, hotkeys, and remaining desktop polish are still in progress. The Tauri/WebView2 app in `app/src-tauri` is the previous prototype and is not the current build.
>
> Renderer decision: [docs/decisions/0001-renderer.md](docs/decisions/0001-renderer.md). Native measurements and review screenshots: [Phase A](docs/phase-a/RESULTS.md), [Phase B](docs/phase-b/RESULTS.md), and [Phase C](docs/phase-c/RESULTS.md).

![Native Card preview](docs/phase-b/card-100.png)

Offscreen render over a neutral review backdrop. The real widget is a transparent layered window on the desktop.

## Current native widget

- Frameless, always-on-top widget with Island, Card, and Lyrics-only modes
- Event-driven Windows SMTC now-playing; no polling loop
- Time-synced lyrics from [LRCLIB](https://lrclib.net), with plain-text, instrumental, loading, empty, unsynced, and offline states
- Cached static background and shaped lyric masks; a lyric transition redraws only the text layer
- Five surfaces: Solid, Transparent, Acrylic, Ice, and Liquid glass. The three textured surfaces share one CPU wallpaper backdrop pipeline, with no DWM acrylic.
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
cargo run -p iceberg-widget -- --demo --background=liquid
```

Render review screenshots and exit:

```powershell
cargo run -p iceberg-widget -- --screenshot=../docs/phase-c
```

Run the warm transition and background benchmark:

```powershell
cargo run -p iceberg-widget -- --bench=../docs/phase-c/bench-100-acrylic.json --background=acrylic --scale=100
```

The release executable is `app/target/release/iceberg.exe`. For a packaged build, `app/assets/fonts` is copied to a `fonts` folder next to the executable. The installer and settings app are not available yet; do not use the old `npm run tauri build` path for the native widget.

## Measurements

Windows 11, release build, default Lato, 12 lyric transitions at 200 ms with 240 ms spacing, 2.9 s transition window plus a 2 s idle window after a 200 ms settle. Three runs per surface; the table shows the median. Total CPU is normalized across the build machine's 16 logical processors. Working set and private bytes are for `iceberg.exe`; there are no child processes. The warm background column is the rebuild after a move settle, with the font and meta masks already warm; the value after the slash is the backdrop pipeline alone.

| Surface | Scale | Window | Transition CPU total | Idle | Working set / private | Warm background / pipeline |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Solid | 100% | 388 x 245 | 0.54% | 0% | 20.7 / 6.9 MB | 7.3 / 0 ms |
| Transparent | 100% | 388 x 233 | 0.54% | 0% | 20.6 / 6.9 MB | 3.2 / 0 ms |
| Acrylic | 100% | 388 x 245 | 0.61% | 0% | 24.0 / 9.6 MB | 54.4 / 30.2 ms |
| Ice | 100% | 388 x 245 | 0.54% | 0% | 24.0 / 9.6 MB | 38.5 / 15.8 ms |
| Liquid | 100% | 388 x 245 | 0.51% | 0% | 24.0 / 9.6 MB | 37.7 / 23.2 ms |
| Acrylic | 200% | 776 x 490 | 1.28% | 0% | 30.3 / 14.5 MB | 154.0 / 53.4 ms |
| Ice | 200% | 776 x 490 | 1.25% | 0% | 30.2 / 14.4 MB | 113.9 / 57.1 ms |
| Liquid | 200% | 776 x 490 | 1.52% | 0% | 30.3 / 15.1 MB | 207.5 / 103.9 ms |

- All transition CPU totals are under 3%; the highest one-core median is 24.25% at 200% Liquid. Full one-core ranges and raw JSON are in [Phase C results](docs/phase-c/RESULTS.md).
- Background rebuilds during the 12 transitions: **0**. Idle CPU is 0% for every surface after the settle.
- The 200% Liquid rebuild is the most expensive opt-in surface. Solid is the default and rebuilds in 7.3 ms at 100%; the textured surfaces are selected explicitly.
- The widget stays under the 40 MB software budget at 30.3 MB working set. Release executable **3.23 MiB**; bundled fonts **2.85 MiB**. The installer is not built yet.
- The tray-only process loads no wallpaper and no font data. The last clean tray-only measurement was 3.3 MB working set / 2.6 MB private; window teardown drops the wallpaper proxy and font cache. A fresh tray-only sample on this machine was blocked by a live media session.
- Phase B's 100/150/200 scale sweep for the solid surface is in [Phase B results](docs/phase-b/RESULTS.md); real mixed-DPI monitor moves remain untested.

## Backgrounds

- **Solid** uses the configured color and opacity. **Transparent** leaves the window clear so the actual desktop shows through. **Acrylic**, **Ice**, and **Liquid** mirror the Windows wallpaper behind the card through a shared CPU pipeline: crop, blur, tint/frost/noise, optional liquid refraction and highlights, then an antialiased rounded shape.
- The wallpaper is read from the Windows wallpaper path, decoded with WIC, and kept as a 960 px proxy. Only a textured surface loads it. The widget cover-crops that proxy to the monitor and window, so fit, tile, stretch, and span wallpaper modes are approximate rather than exact mirrors.
- The backdrop is recomputed after a 120 ms move settle, on resize, and on monitor change. The static card layer and title/artist masks are cached; a lyric transition still redraws only the text layer. Textured processing is capped at 400 px and then sampled into the card, which keeps the 200% cost down without changing the 100% path.
- No DWM acrylic, `SetWindowCompositionAttribute`, or `DwmEnableBlurBehindWindow` calls are used. Live desktop capture is out of v1; the `live` backdrop source falls back to the wallpaper. Wallpaper changes are picked up on window create/show/move, not while the widget sits still. Per-monitor wallpapers are not mirrored yet.
- The Phase C screenshots use a generated review wallpaper so the build machine's desktop is not published. See [docs/phase-c](docs/phase-c/README.md).

## Typography

The stock families are Lato, Poppins, Abril Fatface, Barlow Condensed, Amatic SC, Space Mono, Varela Round, and Lobster. Each is SIL Open Font License 1.1; the license texts and sources are in [app/assets/fonts](app/assets/fonts/README.md).

User fonts are read from `%LOCALAPPDATA%\Iceberg\fonts`. The core validates the file, rejects empty or oversized files, reads family/subfamily/metrics/variable axes, and falls back to Lato if a requested family cannot be resolved. Weight uses the family's bold face when available and a synthetic embolden above 600 when it is not; italic is a synthetic shear so the control remains real for families without an italic file.

Word timings from enhanced LRC are parsed by the core but are not highlighted yet.

The font previews use the same card at every family: 388 x 245 at 100%. Optical size and baseline are normalized from each font's cap height and ascender; line spacing stays on the user's line-height grid so the widget does not resize when the family changes.

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
- The textured surfaces read the current Windows wallpaper file locally through WIC. The widget does not capture other windows or the live desktop.
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
- Word-level lyric highlighting
- Exact wallpaper fit/tile/span mapping, per-monitor wallpapers, live wallpaper-change detection, and live desktop capture
- A cheaper 200% Liquid rebuild; the current opt-in surface is 207.5 ms after a move settle

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
