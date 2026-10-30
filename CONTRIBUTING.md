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

Run the widget with the demo track, render review screenshots, or run the warm benchmark:

```powershell
cargo run -p iceberg-widget -- --demo
cargo run -p iceberg-widget -- --demo --background=liquid
cargo run -p iceberg-widget -- --screenshot=../docs/phase-c
cargo run -p iceberg-widget -- --bench=../docs/phase-c/bench-100-acrylic.json --background=acrylic --scale=100
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
- a 100% preview in `docs/phase-c` when typography changes.

Do not commit user-imported font files, font collections, or fonts with a copyleft license. Validate every font at the boundary; the core rejects empty, oversized, and unreadable files.

## Backgrounds

The widget has five surfaces: Solid, Transparent, Acrylic, Ice, and Liquid. The three textured surfaces share the CPU pipeline in `app/widget/src/render.rs`; `app/widget/src/backdrop.rs` reads and crops the Windows wallpaper through WIC.

- No DWM acrylic, `SetWindowCompositionAttribute`, or `DwmEnableBlurBehindWindow`. The accepted architecture is wallpaper capture, blur, tint, noise, and optional refraction on the CPU.
- Live desktop capture is out of v1. A `live` backdrop source falls back to the wallpaper.
- Only the current wallpaper proxy is kept; it is dropped with the window. Textured processing is capped at 400 px and sampled up into the card.
- The backdrop rebuilds after the 120 ms move settle, on resize, and on monitor change. Title/artist masks are cached separately from the shaped lyric lines.
- Report the warm background rebuild and the backdrop-pipeline subset when changing windows, blur, tint, or refraction. The 200% Liquid rebuild is the most expensive opt-in path.

If a surface cannot meet the performance budget, make it opt-in or cut it.

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

CPU is measured for the whole process tree. Report the surface size, scale, working set, private bytes, and transition/idle CPU. Use the bundled benchmark and the same 12 x 200 ms transitions when comparing changes. The widget must not create a continuous render loop; text transitions redraw only the text layer, and the static background layer rebuilds only on a real layout, art, wallpaper, or monitor change. The benchmark reports the warm background rebuild and the backdrop-pipeline subset separately. Background rebuild is a one-time move cost, not idle work; the 100% Solid rebuild is currently 7.3 ms and the 200% Liquid rebuild is 207.5 ms.

Do not add:

- Polling loops or per-frame timers when idle, paused, or hidden
- Continuous animation loops
- WebView2, remote content, UI kits, icon packs, or web fonts to the widget
- DWM acrylic or live desktop capture
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
app/widget/            native window, renderer, wallpaper backdrop, tray, diagnostics
app/assets/fonts/      bundled OFL fonts and licenses
app/src-tauri/         previous Tauri prototype (reference only)
docs/                  design notes, decisions, phase screenshots and measurements
```

`site/` is a local-only promotional site and is not part of the published app.

## License

By contributing, you agree that your contribution is licensed under the MIT license in [LICENSE](LICENSE).
