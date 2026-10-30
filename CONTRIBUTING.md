# Contributing to Iceberg

Thanks for helping. Iceberg is intentionally small and fast; please keep changes focused.

## Workspace

```text
app/core/              shared config, SMTC, LRCLIB, cache, fonts, presets, IPC schema
app/widget/            native layered window, renderer, wallpaper capture, tray, hotkeys
app/settings/          Tauri 2 settings companion and vanilla TypeScript UI
app/settings/src-tauri settings backend, commands, native file dialog, release bundle config
app/xtask/             native preview generator and release packaging helper
app/assets/            fonts, licenses, preset/surface thumbnails, tray icon
app/src-tauri/         previous Tauri prototype (reference only, excluded from the workspace)
```

The widget is native Rust. Only the settings companion uses WebView2. Do not add WebView2, remote content, UI kits, icon packs, or web fonts to the widget.

## Prerequisites

- Windows 10/11 x64
- Rust stable 1.90 or newer
- Node.js 22 LTS and npm for the settings frontend and packaging
- WebView2 runtime for running the settings window

## Build and test

```powershell
cd app
npm --prefix settings ci
npm --prefix settings run build
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

The Rust `cargo test` suite does not need the settings frontend, but the Tauri crate's build script expects `app/settings/dist` to exist. Build the frontend before a clean workspace clippy/test run.

Run the native widget with the built-in demo:

```powershell
cargo run -p iceberg-widget -- --demo
cargo run -p iceberg-widget -- --demo --background=liquid
```

Run the settings app in development. Build the widget first; the settings backend looks for `iceberg.exe` next to the debug settings executable:

```powershell
cargo build -p iceberg-widget
npm --prefix settings run tauri -- dev --no-watch
```

Regenerate the native preset, surface, and font specimen assets when their renderers change:

```powershell
cargo xtask
```

## Packaging

`cargo xtask package` builds the frontend, both release executables, the portable folder at `app/dist/Iceberg`, and the NSIS installer. The installer step uses the Tauri CLI and its release config, including the widget sidecar and assets; the first run downloads the Tauri NSIS tool. Installers and `dist/` are never committed. The release workflow publishes the installer and a portable ZIP with SHA-256 checksums.

The widget requires no installer resources beyond its asset folder. The settings bundle copies `app/assets` to the installed `resources/assets`; the core looks next to the executable and under `resources/assets` so both the portable and installed layouts work.

## Settings and IPC

- The settings process is created on demand and exits when its window closes.
- The widget owns a per-session named pipe. The settings holds one persistent connection and sends framed JSON messages; it is not a polling channel.
- `app/core/src/ipc.rs` owns the schema and framing. Config writes are validated and saved by the widget before the response.
- The settings frontend calls only app commands. It has `core:default` Tauri permissions and no filesystem, shell, arbitrary HTTP, or remote content permissions.
- The font picker must use the native specimens rendered by `iceberg --font-previews`; do not replace them with WebView `@font-face` previews. The specimen path is the widget's cosmic-text/swash rasterizer.
- The widget remains the live preview. Keep the demo/current-track behavior when Settings opens and the cleanup when it closes.
- The focus refresh must not overwrite an edit that started while the request was in flight.

## Fonts

The stock families and their OFL licenses live in `app/assets/fonts`. New stock fonts need:

- an OFL-compatible license file next to the font;
- a build with Regular and, where available, Bold;
- an entry in `STOCK_FONTS` in `app/core/src/fonts.rs`;
- a 100% preview in `docs/phase-c` when typography changes.

Do not commit user-imported font files, font collections, or fonts with a copyleft license. Validate every font at the boundary; the core rejects empty, oversized, and unreadable files. Imported fonts are copied under generated names inside `%LOCALAPPDATA%\Iceberg\fonts`.

## Backgrounds

The widget has five surfaces: Solid, Transparent, Acrylic, Ice, and Liquid. The three textured surfaces share the CPU pipeline in `app/widget/src/render.rs`; `app/widget/src/backdrop.rs` reads and crops the Windows wallpaper through WIC.

- No DWM acrylic, `SetWindowCompositionAttribute`, or `DwmEnableBlurBehindWindow`. The accepted architecture is wallpaper capture, blur, tint, noise, and optional refraction on the CPU.
- Live desktop capture is out of v1. A `live` backdrop source falls back to the wallpaper.
- Only the current wallpaper proxy is kept; it is dropped with the window. Textured processing is capped at 400 px and sampled up into the card.
- The backdrop rebuilds after the 120 ms move settle, on resize, and on monitor change. Title/artist masks are cached separately from the shaped lyric lines.
- Report the warm background rebuild and the backdrop-pipeline subset when changing windows, blur, tint, or refraction.

If a surface cannot meet the performance budget, make it opt-in or cut it.

## Performance budget

These are hard targets for the native widget. Measure before and after changing layout, rendering, timers, fonts, window code, hotkeys, or the settings pipe.

| State | Budget |
| --- | --- |
| Tray only, no window | under 10 MB working set |
| Widget visible and playing | under 40 MB working set with software rendering |
| Idle or paused | 0% CPU |
| During a lyric transition | under 3% CPU |
| Installer | under 8 MB |
| Executable | under 5 MB |

CPU is measured for the whole widget process tree. The settings window is a separate transient process and is reported separately when relevant. Use the bundled benchmark and the same 12 x 200 ms transitions when comparing rendering changes. The widget must not create a continuous render loop; text transitions redraw only the text layer, and the static background layer rebuilds only on a real layout, art, wallpaper, or monitor change. The benchmark reports the warm background rebuild and the backdrop-pipeline subset separately.

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
- Keep Windows registry writes scoped to HKCU and reversible.
- One logical change per pull request.

## Commit messages

Write them like a person, not a template. Short, lowercase, no emoji, no trailing period. Say what changed and why if it is not obvious.

Good:

- `fix lyrics drifting after seeking`
- `keep the card size stable across fonts`
- `add stock font metrics`
- `ship the settings process`

Bad:

- `feat(lyrics): implement comprehensive synchronization enhancements`
- `Updated files`
- `fix stuff`

Do not add tool attribution, co-author trailers, or "generated with" footers to commits, PRs, or code.

## Pull request checklist

- What changed and why
- How it was tested (players used, Windows version)
- Memory/CPU numbers if behavior or rendering changed
- `cargo fmt --all --check`, clippy, tests, and `npm --prefix settings run build` all pass
- `CHANGELOG.md` updated for user-visible changes
- No new telemetry or network hosts
- No secrets, user font files, build output, installers, `site/`, or `AGENTS.md` staged
- Settings changes checked in both light and dark Windows themes when visual

## Layout

```text
app/core/              shared config, SMTC, LRCLIB, cache, fonts, presets, IPC
app/widget/            native window, renderer, wallpaper backdrop, tray, hotkeys, diagnostics
app/settings/          Tauri settings companion and TypeScript UI
app/settings/src-tauri settings backend, commands, bundle config
app/assets/            bundled OFL fonts, licenses, previews, tray icon
app/xtask/             preview and packaging helper
app/src-tauri/         previous Tauri prototype (reference only)
docs/                  design notes, decisions, phase screenshots and measurements
```

`site/` is a local-only promotional site and is not part of the published app.

## License

By contributing, you agree that your contribution is licensed under the MIT license in [LICENSE](LICENSE).
