# Phase B: rendering, typography, states

Phase B adds the stock font set, lazy font resolution, optical/baseline normalization, synthetic weight and italic, fade transitions, and the config migration that moves previous system-font defaults onto the stock set. The states from Phase A are unchanged in structure and now render in the resolved stock family.

All screenshots in this folder are offscreen renders over a neutral review backdrop, not a design sign-off. The real widget is a transparent layered window.

## Stock fonts

Eight OFL 1.1 families are bundled under `app/assets/fonts`. Each family folder has its own `OFL.txt`; `app/assets/fonts/README.md` lists the sources and categories.

| Family | Category | Faces | Bytes |
| --- | --- | --- | ---: |
| Lato | Clean sans | Regular, Bold | 1,313,112 |
| Poppins | Geometric sans | Regular, Bold | 316,312 |
| Abril Fatface | Serif display | Regular | 67,364 |
| Barlow Condensed | Condensed | Regular, Bold | 212,392 |
| Amatic SC | Handwritten | Regular, Bold | 307,804 |
| Space Mono | Monospace | Regular, Bold | 197,588 |
| Varela Round | Rounded | Regular | 132,748 |
| Lobster | Display | Regular | 406,076 |

Total font assets: **2,989,749 bytes (2.85 MiB)** including license texts.

`FontLibrary` resolves a family lazily. It keeps only the current entry in its cache; the renderer removes the previous family's fontdb faces when the active family changes. A renderer test switches Lato, Poppins, Abril Fatface, and Space Mono in one renderer and asserts that no more than the current family's faces remain loaded. A tray-only session never resolves a family, so it carries no font bytes.

User fonts are discovered in `%LOCALAPPDATA%\Iceberg\fonts` by family name, validated with the existing core parser, and cached one at a time. TTF and OTF are supported. WOFF and WOFF2 decoding is not implemented yet; that lands with the settings upload flow.

## Metrics and layout

- Optical size uses the font's cap-height ratio against the reference, clamped to 0.5–2.0x.
- The baseline offset is derived from the font's ascender and the scaled size, applied to lyric, title, artist, and state text.
- Line spacing follows `typography.line_height` on the base grid, not each family's natural leading. This keeps the card at 388 x 245 at 100% for every stock family instead of resizing by up to 24 px when the font changes.
- The font's measured line-height correction is still stored in `FontMetrics` for diagnostics, but the widget layout does not substitute it for the user's line-height setting.

The eight `font-*-100.png` previews all have a 388 x 245 surface. They use the same demo track and active line, so cap height, baseline, and line positions can be compared directly.

Weight selects the family's bold face when one exists. For single-weight families, weights at 600 and above use a synthetic one-pixel embolden so the control still changes the text; 300–500 stay on the regular face. Italic is a synthetic shear applied when compositing the mask, so it works for families without an italic file. Variable-font axes are parsed into `FontInfo` but are not instantiated by the renderer.

## Rendering

- The resolved family, regular data, and bold data are passed to the renderer through the view; the font is loaded into fontdb only when the family changes.
- The renderer cache is cleared on a family change. The previously loaded faces are removed with `fontdb::Database::remove_face`.
- Text masks now scale premultiplied color by opacity only. The previous code multiplied RGB by alpha a second time, which made antialiased edges too thin.
- Fade transitions keep the target line grid and crossfade opacity; slide uses the eased float index; off jumps without a transition. Only the text layer is redrawn during any transition.
- Title, artist, lyric, and state masks use the same normalized metrics and the same synthetic weight/italic controls.

## Config

`CONFIG_VERSION` is now 2. Existing font names are mapped to stock families on load:

| Previous | Migrated |
| --- | --- |
| Segoe UI, Segoe UI Variable Display/Text | Lato |
| Georgia | Abril Fatface |
| Bahnschrift | Barlow Condensed |
| Consolas, Cascadia Mono/Code | Space Mono |

The default typography family is Lato. Paper uses Abril Fatface and Quiet uses Space Mono. Per-song family overrides migrate through the same table and still resolve independently of the lyric offset.

## Screenshots

19 PNGs, 397,289 bytes total. The largest is `card-200.png` at 49,546 bytes; every file is under 50 KB.

- `card-100.png`, `card-150.png`, `card-200.png`: Card scale sweep
- `lyrics-100.png`, `island-100.png`: other modes
- `state-empty-100.png`, `state-loading-100.png`, `state-nolyrics-100.png`, `state-instrumental-100.png`, `state-unsynced-100.png`
- `card-arabic-100.png`: Arabic shaping with the normal system fallback
- `font-<family>-100.png`: eight stock families, same card and line

## Measurements

Release build, Windows 11, default Lato, 12 lyric transitions at 200 ms with 240 ms spacing, 2.9 s transition window and 2.0 s idle window. Three runs per scale; the table shows the median transition CPU and working set, and the observed idle range. Total CPU uses the build machine's 16 logical processors. The JSON files are the median run for each scale; the ranges below are across all three.

| Scale | Surface | Transition CPU (one core / total) | Idle CPU | Working set / private | Background rebuilds |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 100% | 388 x 245 | 5.39% / 0.34% | 0% | 20.6 / 6.9 MB | 0 |
| 150% | 582 x 368 | 8.08% / 0.51% | 0% | 23.2 / 9.1 MB | 0 |
| 200% | 776 x 490 | 12.93% / 0.81% | 0–0.78% / 0–0.05% | 26.8 / 12.0 MB | 0 |

- One-core transition ranges across the three runs: 4.85–5.93% at 100%, 7.54–8.62% at 150%, 12.93–14.55% at 200%.
- Total transition CPU was 0.30–0.91% at every tested scale.
- Idle CPU was 0% at 100% and 150%. At 200%, two of three 2 s idle samples read 0.78% of one core (0.05% total); one read 0.
- The static background rebuilds once before the transition window and zero times during the 12 transitions.
- The 150% and 200% rows use `--scale`; the machine has one 100% monitor. Real monitor moves at mixed DPI remain a Phase E test.
- Release executable: **3,358,720 bytes (3.20 MiB)**. Font assets: **2.85 MiB**. Raw widget resources are 6.05 MiB; the installer size depends on compression and the settings app, which do not exist yet.
- A live media session at the 100% scale produced a 30.0 MB working set / 8.1 MB private sample with the window open. That is a sanity sample, not a controlled bench; it stays under the 40 MB visible budget.
- The last clean tray-only measurement was 3.3 MB working set / 2.6 MB private in Phase A. A fresh tray-only sample on this machine was not possible because a media session kept the widget window open. Destroying the window clears the font library and active entry, so returning to tray mode after playback drops the font bytes as well. The renderer unload test covers family switches while the window is alive.

## Verification

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace`: 66 passed, 1 ignored (the live LRCLIB roundtrip)
- `cargo build --release -p iceberg-widget`

The stock font test parses all eight families and their bold faces, checks the license files, and verifies the family names. The renderer test loads four stock families into fontdb, shapes the same line, and checks both distinct widths and face eviction.

## Deferred

- WOFF/WOFF2 import and the settings-side font upload UI
- Font picker UI in the settings app; the stock catalog and lazy resolver are ready
- Packaging `app/assets/fonts` next to the installed widget executable
- Variable-font weight axes
- Word-level lyric highlighting (word timings are parsed but only line-level opacity is rendered)
- Real 150%/200% hardware, Windows 10, two-monitor, mixed-DPI move, and the 20-minute leak run
- Live-capture backdrop (still out of v1 unless it stays under 3% CPU on battery)
