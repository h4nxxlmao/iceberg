# Changelog

All notable changes to Iceberg are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Native widget workspace (`app/core`, `app/widget`) with per-monitor DPI v2, exact tool-window styles, event-driven SMTC, LRCLIB enhanced word timings, font metric normalization, six presets, cached background and lyric layers, and a lazy tray-only mode.
- Renderer decision after the Slint/wgpu/CPU spike: the CPU layered-window renderer in [docs/decisions/0001-renderer.md](docs/decisions/0001-renderer.md).
- Phase A review screenshots and release measurements in [docs/phase-a/](docs/phase-a/).
- Eight bundled OFL font families (Lato, Poppins, Abril Fatface, Barlow Condensed, Amatic SC, Space Mono, Varela Round, Lobster) with lazy per-family loading and licenses in `app/assets/fonts`.
- Lazy `FontLibrary` resolution for stock and user fonts, cap-height optical scaling, baseline normalization, per-track font selection, synthetic bold above 600 when a family has no bold face, and synthetic italic.
- Config v2 migration from the previous system-font defaults to the stock set.
- Fade transitions that hold the target line grid, alongside slide and off.
- Phase B typography previews and release measurements in [docs/phase-b/RESULTS.md](docs/phase-b/RESULTS.md).

### Changed

- The default widget font is now Lato instead of Segoe UI Variable Display. Line spacing stays on the user's line-height grid so the card does not resize when the family changes.
- The Paper preset uses Abril Fatface and Quiet uses Space Mono.
- Text masks are composited with their premultiplied alpha, fixing thin antialiased edges.

### Fixed

- Font name-table lookup now keeps scanning records when the first matching record cannot be decoded, so Lato and other stock fonts resolve their family names correctly.

### Performance

- Phase B release measurements (median of three) at 100/150/200%: 5.39/8.08/12.93% of one core during 12 cached-text transitions (0.34/0.51/0.81% of total CPU), 20.6/23.2/26.8 MB working set, 6.9/9.1/12.0 MB private. Idle CPU is 0% at 100/150%; two 200% samples read 0.78% of one core. Release executable 3.20 MiB plus 2.85 MiB of font assets; background rebuilds during transitions remain 0.

### Changed (previous Tauri prototype)

- Rebuilt the widget and settings around the Glacial instrument direction: ink and bone surfaces, a signal orange accent, square controls, a numbered settings index, and a diagnostics ledger.
- Replaced the old theme presets with Glacier, Paper, Graphite, Ember, Frost, and Cover.
- Removed the static equalizer and the generated triangle mark from the settings UI.
- Fixed low-contrast lyric states and added loading and error states to the settings sheet.

### Performance (previous Tauri prototype)

- Release measurement after the redesign: 59.2 MB visible, 11.0 MB hidden, 0.78% of one core in steady lyric playback, 4.07 MB executable, 1.69 MiB installer.
- Phase A native measurements are in [docs/phase-a/RESULTS.md](docs/phase-a/RESULTS.md).

## [0.1.0] - 2026-10-27

### Added

- Tray-only Windows start with a transparent, always-on-top widget created only when playback starts
- Windows SMTC now-playing support driven by media events, with title cleanup, album art, and dominant-color accents
- LRCLIB synced and plain lyrics with a 5 second timeout, one retry, disk cache, 24 hour not-found cache, and 30 MB cap
- LRC parser with multiple timestamps, metadata offset, instrumental detection, and manual `.lrc` import
- Island, Card, and Lyrics-only modes with native acrylic/mica options, opacity, fonts, colors, border, shadow, and radius
- Config validation, safe defaults, debounced writes, per-song lyric offsets, themes, reset, and portable mode
- Tray menu, single-instance behaviour, autostart, global hotkeys, click-through, edge snapping, and per-monitor positions
- Lazy settings window with live preview, wrong-lyrics search, theme import/export, diagnostics, and cache controls
- Zero-webview idle mode, WebView2 suspend with destroy fallback, hidden working-set trimming, and EcoQoS
- Event-driven lyric sync, a stepped low-CPU progress bar, and recycled 3–5 line lyric rendering
- Fullscreen auto-hide, quick peek, idle fade, lyric copy, and clean loading/offline/instrumental states

### Performance

- 66.0 MB private working set while visible and playing
- 13.0 MB after the hidden teardown; 1.7 MB tray-only
- 0.08 CPU-seconds per 12 seconds while lyrics are shown (about 0.7% of one core)
- 4.07 MB executable; 1.69 MiB NSIS installer with LZMA compression and the WebView2 bootstrapper
