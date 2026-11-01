# Roadmap

Iceberg is a small project with a hard performance budget. This is a direction, not a promise; priorities can change when a bug matters more.

## Next

- Validate the release build on Windows 10, two-monitor mixed-DPI setups, and a longer idle/playback leak run.
- Add word-level highlighting for enhanced LRC files. Parsing already exists; the renderer does not use it yet.
- Improve wallpaper mapping for fit, tile, stretch, and span modes, and detect wallpaper changes while the widget is stationary.
- Keep the settings window keyboard-accessible and usable in both Windows light and dark themes.

## Later

- Per-monitor wallpaper handling on multi-monitor desktops.
- More preset and surface work without adding a heavier rendering path.
- A signed Windows build, if the project can support code signing without hiding the build process.

## Not planned

- Accounts, telemetry, analytics, or crash reporting.
- Remote content or a WebView inside the widget.
- Additional lyric or metadata network hosts beyond the LRCLIB/lrcmux paths in the default build.
- Polling loops, continuous animation while idle, or UI frameworks that would break the size and memory targets.

Have a use case that is not listed? Open a feature request with the problem, not only the proposed control. The performance budget and the existing architecture are part of the decision.
