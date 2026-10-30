# Phase C: wallpaper surfaces

Phase C adds the five surfaces: Solid, Transparent, Acrylic, Ice, and Liquid glass. Acrylic, Ice, and Liquid share one CPU backdrop pipeline built from the current Windows wallpaper. There is no DWM acrylic, no live desktop capture, and no new dependency.

The screenshots in this folder are offscreen renders over a neutral review backdrop and a generated wallpaper. They are review material, not a design sign-off; the real widget is a transparent layered window on the desktop.

## Wallpaper source

`app/widget/src/backdrop.rs` reads the wallpaper path with `SystemParametersInfoW(SPI_GETDESKWALLPAPER)` and decodes it with WIC through the pinned `windows` crate. The decoder converts to BGRA, keeps a proxy no larger than 960 px, and converts to straight RGBA for the renderer. A COM guard balances `CoInitializeEx` and `CoUninitialize`; `RPC_E_CHANGED_MODE` is accepted because the event-loop thread may already have COM.

The widget cover-crops the proxy to the full monitor and then to the window. The monitor rect comes from the current `MonitorHandle`; the window rect is the anchored position and current metrics. Fit, tile, stretch, and span wallpaper modes are approximated as cover; per-monitor wallpapers are not mirrored. The proxy is loaded only when the surface is Acrylic, Ice, or Liquid, and is dropped with the window. A wallpaper with no file path falls back to the flat tint.

The wallpaper change check runs when the window is created, shown, or moved. A change while the widget sits still is not detected until one of those events.

## Shared pipeline

1. Cover-crop the wallpaper to the window.
2. Downsample to at most 400 px on the longest side. The 100% window is smaller than the cap and takes the unchanged full-resolution path.
3. Apply two separable box-blur iterations (four passes) with a surface-specific radius.
4. Tint and texture:
   - Acrylic mixes toward `acrylic_tint` at `acrylic_tint_strength` and adds deterministic grain from `acrylic_noise`.
   - Ice blends a little of the unblurred crop back (`ice_clarity`), desaturates, mixes toward `ice_tint` using `ice_frost`, and adds frost grain.
   - Liquid mixes toward `solid_color`, then refracts the edge with `liquid_refraction`, `liquid_bevel`, and `liquid_highlight`; `liquid_chromatic` splits the red and blue samples for an edge fringe.
5. Sample the processed image through the antialiased rounded-card shape. The border is drawn after the fill; the shadow is drawn before it.
6. Cache the static background layer and the title/artist masks. A lyric transition still redraws only the text layer. A move marks the backdrop dirty after a 120 ms settle.

Solid and Transparent bypass the wallpaper path. The Transparent surface leaves the card clear; the benchmark's 388 x 233 geometry is the config without the progress row, while the Clear preset screenshot is 360 x 212 because it also has no shadow margin. The Frost preset now uses ink text on the light ice surface.

The `--background=solid|transparent|acrylic|ice|liquid` diagnostic selects a surface for the demo and benchmark. `BackdropSource::Live` is treated as Wallpaper because live capture is out of v1.

## Screenshots

25 PNGs, 750.6 KB total. The largest is 101.2 KB; each file is under 500 KB.

- `background-solid-100.png`
- `background-transparent-100.png`
- `background-acrylic-100.png`
- `background-ice-100.png`
- `background-liquid-100.png`
- `background-liquid-chromatic-100.png`
- `card-*`, `lyrics-*`, `island-*`, `state-*`, `card-arabic-*`, and `font-*`: current Phase B context, rendered on a solid card

The background previews use a generated mineral wallpaper with teal, signal orange, pale, and ink fields. The build machine's actual wallpaper is not published. The transparent and solid previews show the review backdrop rather than a desktop because these are offscreen renders.

## Measurements

Windows 11, release build, default Lato, 12 lyric transitions at 200 ms with 240 ms spacing. The transition window is 2.9 s. The idle window is 2.0 s after a 200 ms settle so the trailing redraw is not counted. Three runs per surface; the table shows the median transition CPU, idle, working set/private, and the warm background rebuild. "Warm background" is the second rebuild after the font, buffers, and title/artist masks are warm; the backdrop pipeline is the `paint_backdrop` subset. The first startup rebuild is noted below.

| Surface | Scale | Window | Transition one core | Transition total | Idle | WS / private | Warm background / pipeline |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Solid | 100% | 388 x 245 | 8.62% | 0.54% | 0% | 20.7 / 6.9 MB | 7.3 / 0 ms |
| Transparent | 100% | 388 x 233 | 8.62% | 0.54% | 0% | 20.6 / 6.9 MB | 3.2 / 0 ms |
| Acrylic | 100% | 388 x 245 | 9.70% | 0.61% | 0% | 24.0 / 9.6 MB | 54.4 / 30.2 ms |
| Ice | 100% | 388 x 245 | 8.62% | 0.54% | 0% | 24.0 / 9.6 MB | 38.5 / 15.8 ms |
| Liquid | 100% | 388 x 245 | 8.08% | 0.51% | 0% | 24.0 / 9.6 MB | 37.7 / 23.2 ms |
| Acrylic | 200% | 776 x 490 | 20.47% | 1.28% | 0% | 30.3 / 14.5 MB | 154.0 / 53.4 ms |
| Ice | 200% | 776 x 490 | 19.94% | 1.25% | 0% | 30.2 / 14.4 MB | 113.9 / 57.1 ms |
| Liquid | 200% | 776 x 490 | 24.25% | 1.52% | 0% | 30.3 / 15.1 MB | 207.5 / 103.9 ms |

- One-core transition ranges across the three runs: Solid 100 8.62–9.16%, Transparent 5.93–9.16%, Acrylic 100 7.00–12.93%, Ice 100 8.08–10.78%, Liquid 100 8.08–12.39%, Acrylic 200 19.40–21.01%, Ice 200 19.40–22.63%, Liquid 200 19.94–25.86%.
- Every surface stays below 1.6% of total CPU during the transitions and below 3% as required. Idle CPU is 0% after the settle.
- Background rebuilds during the 12 transitions: **0** for every run. The 200 ms settle and the text-only transition were both verified by the raw JSON.
- The first background rebuild (startup, includes initial font upload and buffer warm-up) ranges from 41.4 ms Solid 100 to 216.6 ms Liquid 200. The warm values above are the meaningful move cost.
- The 400 px cap improves the 200% textured path but does not remove the full-window composite; Liquid 200 remains the most expensive opt-in surface at 207.5 ms after a settle. The 100% Solid default is 7.3 ms.
- Release executable: **3,390,976 bytes (3.23 MiB)**. Font assets: **2.85 MiB**. Raw widget resources are **6.09 MiB**; the installer and settings app do not exist yet.
- The wallpaper proxy adds roughly 2 MB while a textured surface is active. The 40 MB visible budget is met at 30.3 MB working set and 15.1 MB private at 200%.
- Tray-only still performs no wallpaper or font work; the proxy and both caches are dropped with the window.

## Verification

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace`: 45 core + 29 widget tests passed, 1 live LRCLIB test ignored
- `cargo build --release -p iceberg-widget`

The new tests cover WIC proxy-independent crop math, RGBA validation, blur softening, rounded-image clipping, textured card painting, and Liquid refraction length. The benchmark JSONs in this folder are the median run for each surface; the extra runs behind the ranges were removed after the summary.

## Deferred

- Exact fit/tile/stretch/span wallpaper mapping and per-monitor wallpapers
- Wallpaper change detection while the widget is stationary
- Live desktop capture; it stays out of v1 unless it meets the 3% battery target
- A cheaper 200% Liquid rebuild (chrome cache or lower-resolution composite)
- Settings UI for the five surfaces and their sliders
- Windows 10, two-monitor, real mixed-DPI moves, and the 20-minute leak run
- Word-level lyric highlighting
