# Security Policy

## Supported versions

| Version | Supported |
| --- | --- |
| 0.2.x | Yes |
| Anything older | No |

## Reporting a vulnerability

Please use GitHub's **private vulnerability reporting** on this repository: open the **Security** tab and choose **Report a vulnerability**. Do not open a public issue for a suspected vulnerability.

There is no security email address. GitHub advisories are the private channel.

Please include:

- Iceberg version and Windows version
- What an attacker can do and what access they need
- Steps to reproduce, or a minimal proof of concept
- Any relevant logs or screenshots, with personal data removed

You can expect an acknowledgement within 7 days and an initial assessment within 30 days. If the report is valid, a fix and advisory will be prepared before public disclosure.

## Scope

In scope:

- The native Iceberg widget, the settings companion, and the installer
- The LRCLIB and lrcmux fallback request paths, and cache handling
- The local settings IPC pipe and the settings WebView2 configuration
- Font import, config validation, and the HKCU autostart entry
- Memory-safety issues in the Rust and Windows API code

Out of scope:

- LRCLIB, lrcmux, and the content they serve
- Third-party players and their SMTC implementations
- Windows, WebView2, or GPU driver vulnerabilities with no Iceberg-specific trigger
- Reports that require an already-compromised machine

## Security design notes

- The widget is native Rust. It creates a layered window directly and has no WebView or browser engine.
- Iceberg listens to Windows SMTC locally. It does not read player credentials, cookies, or files.
- Iceberg's own network requests go to `lrclib.net` and, as a fallback when LRCLIB is unavailable or has no match, the LRCLIB-compatible endpoint at `api.lrcmux.dev`. Both use HTTPS. Responses are size-limited and parsed defensively.
- The settings companion uses WebView2 only while the window is open. It loads bundled local assets under a strict CSP; there is no remote content, asset protocol, or eval.
- The settings frontend has no filesystem, shell, or HTTP permission. It calls a small set of app commands: read/update config, read fonts and preview images, import a font through a native file dialog, and reveal the widget.
- Lyrics, titles, artists, and imported font family names are untrusted. The widget renders lyric text natively; the settings UI inserts display text as text nodes or escaped HTML. Images are native renderer output or local font data.
- The settings process and widget talk over a per-session named pipe. Any process in the same interactive Windows session can open that pipe and ask the widget to change its own appearance; the protocol exposes no secrets, file reads, network access, or privilege escalation. A local attacker who can run code in the session already controls the desktop.
- Config, cache, and imported fonts live in Iceberg's own folders. The config is validated and clamped on load and on every IPC update. Font files are size-limited, parsed, and written under generated names inside the app's font folder; path traversal is rejected.
- Tauri capabilities for the settings window are `core:default` only. No filesystem, shell, or arbitrary HTTP permissions are exposed to the frontend.
- Autostart writes only to `HKCU\Software\Microsoft\Windows\CurrentVersion\Run\Iceberg` and can be turned off in Settings.
- The old Tauri prototype under `app/src-tauri` is reference material and is not part of the built or published app.

## Dependency and release checks

CI runs Rust formatting, clippy, tests, and both release builds on Windows. The security workflow runs `cargo audit` and `npm audit --omit=dev` weekly and on lockfile changes. Releases are built from tagged source with the local `xtask`/Tauri NSIS pipeline and published with SHA-256 checksums.
