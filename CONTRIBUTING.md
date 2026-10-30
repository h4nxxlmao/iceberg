# Contributing to Iceberg

Thanks for helping. Iceberg is intentionally small and fast; please keep changes focused.

## Native widget workspace (current)

The rebuild lives in `app/core` (shared media, lyrics, config, fonts, presets) and `app/widget` (the native overlay). Only Rust is required for this part.

```powershell
cd app
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --release -p iceberg-widget
```

The Tauri/WebView2 prototype in `app/src-tauri` is kept for reference until the new settings app replaces it and is excluded from the workspace. The setup and PR checklist below describe that prototype; the native widget's measurements are in [docs/phase-a/RESULTS.md](docs/phase-a/RESULTS.md).

## Prerequisites

- Windows 10/11 x64
- Rust stable (1.90 or newer)
- Node.js LTS and npm (Tauri prototype only)
- Microsoft Edge WebView2 Runtime (included with Windows 11, Tauri prototype only)

## Setup

```powershell
cd app
npm ci
npm run tauri dev
```

Production build:

```powershell
cd app
npm run tauri build
```

The installer is under `app/src-tauri/target/release/bundle/nsis/`.

## Before opening a pull request

Run all of these from `app` unless noted:

```powershell
npm run check
npm run build
cd src-tauri
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

There is also an ignored live LRCLIB test:

```powershell
cargo test lrclib_roundtrip -- --ignored
```

Use it when you touch the network or parser code.

## Performance budget

Iceberg has hard targets. Measure before and after changing window, WebView2, cache, or animation code.

| State | Budget |
| --- | --- |
| Tray only | under 15 MB private working set |
| Visible and playing | under 70 MB |
| Hidden or paused | under 30 MB |
| CPU while lyrics show | under 1% |
| Installer | under 8 MB |

Measure the whole process tree (`iceberg.exe` and every WebView2 child), not just the main process. The README documents the current method and numbers.

Do not add:

- Polling loops or per-frame JavaScript
- Continuous CSS animations
- Remote web content, web fonts, UI kits, icon packs, or React
- Per-frame progress updates
- Dependencies with heavy trees, copyleft licenses, or new network hosts
- Telemetry, analytics, or crash reporting

If a feature cannot fit the budget, make it opt-in and say so, or leave it out.

## Code style

- Follow the existing Rust and Svelte style.
- Comment non-obvious *why*, not *what*. Avoid dead code and commented-out blocks.
- Keep the frontend tiny. Svelte + Vite only; system fonts only.
- Use `// SAFETY:` comments for every unavoidable Windows `unsafe` block.
- Validate config and untrusted data at the boundary.
- One logical change per pull request.

## Commit messages

Write them like a person, not a template. Short, lowercase, no emoji, no trailing period. Say what changed and why if it is not obvious.

Good:

- `fix lyrics drifting after seeking`
- `stop rebuilding the tray menu on every color change`
- `add portable mode`

Bad:

- `feat(lyrics): implement comprehensive synchronization enhancements`
- `Updated files`
- `fix stuff`

Do not add tool attribution, co-author trailers, or "generated with" footers to commits, PRs, or code.

## Pull request checklist

- What changed and why
- How it was tested (players used, Windows version)
- Memory/CPU numbers if behavior or rendering changed
- `cargo fmt --check`, clippy, tests, frontend check/build all pass
- `CHANGELOG.md` updated for user-visible changes
- No new telemetry or network hosts

## Layout

```
app/src/               widget and settings UI
app/src-tauri/src/     Rust backend
  smtc.rs              Windows media session events
  lyrics.rs            LRCLIB client, LRC parser, manual lyrics
  cache.rs             disk caches and eviction
  window.rs            create/show/hide/suspend/destroy, positions
  tray.rs              tray icon and menu
  hotkeys.rs           global shortcuts
  perf.rs              EcoQoS, working-set trims, diagnostics
  config.rs            config, validation, themes
```

`site/` is a local-only promotional site and is not part of the published app.

## License

By contributing, you agree that your contribution is licensed under the MIT license in [LICENSE](LICENSE).
