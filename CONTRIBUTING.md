# Contributing to Iceberg

Thanks for helping. Iceberg is intentionally small and fast; please keep changes focused.

## Current workspace

The native rebuild lives in `app/core` (shared media, lyrics, config, fonts, presets, IPC schema) and `app/widget` (the native overlay). Only Rust is required for this part.

```powershell
cd app
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --release -p iceberg-widget
```

Run the widget with the demo track, render review screenshots, or run the transition benchmark:

```powershell
cargo run -p iceberg-widget -- --demo
cargo run -p iceberg-widget -- --screenshot=../docs/phase-b
cargo run -p iceberg-widget -- --bench=../docs/phase-b/bench-100.json --scale=100
```

The Tauri/WebView2 prototype in `app/src-tauri` is kept for reference until the settings app replaces it and is excluded from the workspace. Do not add new features to the prototype.

## Prerequisites

- Windows 10/11 x64
- Rust stable (1.90 or newer)
- Node.js LTS and npm only when working on the old prototype

## Fonts

The stock families and their OFL licenses live in `app/assets/fonts`. New stock fonts need:

- an OFL-compatible license file next to the font;
- a build with Regular and, where available, Bold;
- an entry in `STOCK_FONTS` in `app/core/src/fonts.rs`;
- a 100% preview in `docs/phase-b` when typography changes.

Do not commit user-imported font files, font collections, or fonts with a copyleft license. Validate every font at the boundary; the core rejects empty, oversized, and unreadable files.

## Performance budget

These are hard targets for the native widget. Measure before and after changing layout, rendering, timers, fonts, or window code.

| State | Budget |
| --- | --- |
| Tray only, no window | under 10 MB working set |
| Widget visible and playing | under 40 MB working set with software rendering |
| Idle or paused | 0% CPU |
| During a lyric transition | under 3% CPU |
| Installer | under 8 MB |
| Executable | under 5 MB |

CPU is measured for the whole process tree. Report the surface size, scale, working set, private bytes, and transition/idle CPU. Use the bundled benchmark and the same 12 x 200 ms transitions when comparing changes. The widget must not create a continuous render loop; text transitions redraw only the text layer, and the static background layer rebuilds only on a real layout, art, or wallpaper change.

Do not add:

- Polling loops or per-frame timers when idle, paused, or hidden
- Continuous animation loops
- WebView2, remote content, UI kits, icon packs, or web fonts to the widget
- Per-frame progress updates
- Dependencies with heavy trees, copyleft licenses, or new network hosts
- Telemetry, analytics, or crash reporting

If a feature cannot fit the budget, make it opt-in and say so, or leave it out.

## Code style

- Follow the existing Rust style and keep the frontend small.
- Comment non-obvious *why*, not *what*. No dead code or commented-out blocks.
- Use `// SAFETY:` comments for every unavoidable Windows `unsafe` block.
- Validate config and untrusted data at the boundary.
- Keep window styles explicit and verify them read back; do not rely on framework defaults.
- One logical change per pull request.

## Commit messages

Write them like a person, not a template. Short, lowercase, no emoji, no trailing period. Say what changed and why if it is not obvious.

Good:

- `fix lyrics drifting after seeking`
- `keep the card size stable across fonts`
- `add stock font metrics`

Bad:

- `feat(lyrics): implement comprehensive synchronization enhancements`
- `Updated files`
- `fix stuff`

Do not add tool attribution, co-author trailers, or "generated with" footers to commits, PRs, or code.

## Pull request checklist

- What changed and why
- How it was tested (players used, Windows version)
- Memory/CPU numbers if behavior or rendering changed
- `cargo fmt --all --check`, clippy, tests all pass
- `CHANGELOG.md` updated for user-visible changes
- No new telemetry or network hosts
- No secrets, user font files, build output, or `site/` files staged

## Layout

```
app/core/              shared config, SMTC, LRCLIB, cache, fonts, presets, IPC
app/widget/            native window, renderer, tray, diagnostics
app/assets/fonts/      bundled OFL fonts and licenses
app/src-tauri/         previous Tauri prototype (reference only)
docs/                  design notes, decisions, phase screenshots and measurements
```

`site/` is a local-only promotional site and is not part of the published app.

## License

By contributing, you agree that your contribution is licensed under the MIT license in [LICENSE](LICENSE).
