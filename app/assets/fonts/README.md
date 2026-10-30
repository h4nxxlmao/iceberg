# Stock fonts

Iceberg ships a small OFL-licensed font set so the widget looks the same on every
machine and never depends on a system font being installed. Only the selected family is
read into memory.

| Family | Category | Files | License |
| --- | --- | --- | --- |
| Lato | Clean sans | Regular, Bold | OFL 1.1 |
| Poppins | Geometric sans | Regular, Bold | OFL 1.1 |
| Abril Fatface | Serif display | Regular | OFL 1.1 |
| Barlow Condensed | Condensed | Regular, Bold | OFL 1.1 |
| Amatic SC | Handwritten | Regular, Bold | OFL 1.1 |
| Space Mono | Monospace | Regular, Bold | OFL 1.1 |
| Varela Round | Rounded | Regular | OFL 1.1 |
| Lobster | Display | Regular | OFL 1.1 |

Each family folder contains its own `OFL.txt` with the full license text. The fonts are
unmodified copies from the [Google Fonts](https://github.com/google/fonts) repository.

At runtime the widget looks for a `fonts` folder next to `iceberg-widget.exe` first. When
running from the workspace it falls back to this folder. User-imported fonts live in
`%LOCALAPPDATA%\Iceberg\fonts` and are not redistributed.
