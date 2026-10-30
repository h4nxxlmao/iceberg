# Security Policy

## Supported versions

| Version | Supported |
| --- | --- |
| 0.1.x | Yes |
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

- The Iceberg desktop app and its installer
- The LRCLIB request path, cache handling, and `.lrc`/theme import paths
- Tauri capabilities, CSP, and WebView2 configuration
- Memory-safety issues in the Rust and Windows API code

Out of scope:

- LRCLIB itself and the content it serves
- Third-party players and their SMTC implementations
- Windows, WebView2, or GPU driver vulnerabilities with no Iceberg-specific trigger
- Reports that require an already-compromised machine

## Security design notes

- Iceberg listens to Windows SMTC locally. It does not read player credentials, cookies, or files.
- Iceberg's own requests only go to `lrclib.net`, over HTTPS. Responses are size-limited and parsed defensively. WebView2 background networking is disabled and no remote content is loaded.
- The webview loads only bundled local assets under a strict Content Security Policy. There is no remote content, asset protocol, or eval.
- Lyrics, titles, artists, and theme names are untrusted and rendered as text. The app does not use `innerHTML`.
- Config, cache, and WebView2 data live in Iceberg's own folders. Imported files are limited in size; themes are validated before use.
- Tauri capabilities are limited to events and dragging. No filesystem, shell, or arbitrary HTTP permissions are exposed to the frontend.
- Iceberg runs WebView2 in single-process mode for its memory budget. This is not an officially supported Microsoft configuration. The page is local-only, served under CSP, and the app performs no navigation. This trade-off is documented in the README; if you find a concrete exploit path that depends on it, report it privately.

## Dependency and release checks

CI runs Rust formatting, clippy, and tests on Windows. The security workflow runs `cargo audit` and `npm audit --omit=dev` weekly and on lockfile changes. Installers are built from tagged source with the official Tauri action and published with SHA-256 checksums.
