# Changelog

All notable changes to Iceberg are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Changed

- Rebuilt the widget and settings around the Glacial instrument direction: ink and bone surfaces, a signal orange accent, square controls, a numbered settings index, and a diagnostics ledger.
- Replaced the old theme presets with Glacier, Paper, Graphite, Ember, Frost, and Cover.
- Removed the static equalizer and the generated triangle mark from the settings UI.
- Fixed low-contrast lyric states and added loading and error states to the settings sheet.

### Performance

- Release measurement after the redesign: 59.2 MB visible, 11.0 MB hidden, 0.78% of one core in steady lyric playback, 4.07 MB executable, 1.69 MiB installer.

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
