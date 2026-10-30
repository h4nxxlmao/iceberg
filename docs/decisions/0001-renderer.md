# Renderer decision

Phase 0 spike, 2026-10-30. Short version: the widget renders with **winit + UpdateLayeredWindow + cosmic-text/swash on the CPU**. Slint and wgpu were measured and rejected.

Machine: Windows 11 Home Single Language 10.0.26300, 16 logical cores, NVIDIA RTX 3050 6 GB Laptop GPU, no usable integrated GPU, single monitor, 100% DPI. Windows 10 and 150/200% DPI were not available for this spike; they are Phase E checks.

## What existed before

The prototype was a Tauri/WebView2 app. Debug build, demo mode, widget and settings open:

| Metric | Value |
| --- | --- |
| Working set, `iceberg.exe` + WebView2 child | 275 MB |
| Private bytes, same tree | 94.4 MB |
| Idle CPU, 10 s window | ~0.03% of one core |
| Executable | 18.1 MB (debug, no release build or installer present) |

The UI was a sidebar with numbered sections and stacked cards, six theme swatches, 11-28 px lyrics, no live preview, and a duplicated lyric line. It is replaced, not extended.

## Candidates and results

All numbers are the spike process only, measured with `GetProcessMemoryInfo` and `GetProcessTimes`. Screenshots and raw JSON are in `app/spike/*/spike-results/` locally; the throwaway spike code is not committed.

| Candidate | Build | Working set | Private | Idle CPU (one core) | Transition CPU (one core) | Exe |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| Slint femtovg | debug | 69-77 MB | 56-60 MB | 0% | not cleanly measured | n/a |
| Slint software | debug | 24 MB | 3.6 MB | 0% | n/a | n/a |
| Slint Skia | debug | 300-408 MB | 326-416 MB | 0% | n/a | n/a |
| winit + wgpu + glyphon | release | 152-169 MB | 314-321 MB | 0.78% | 20.83% | 4.66 MB |
| **winit + UpdateLayeredWindow + cosmic-text/swash** | **release** | **18.1-21.0 MB** | **3.5-3.7 MB** | **0-1.56%** | **13.02%** | **1.87 MB** |

CPU is percent of one logical core over the measured window. The target "under 1% idle, under 3% during transitions" is read as whole-system CPU; on 16 cores 1.56% of one core is about 0.10% total.

### Gate 1: transparent borderless overlay

- **Software (chosen):** layered window plus `UpdateLayeredWindow` with a 32-bit premultiplied DIB. Over a two-colour test card, the widget region variance is 3064.1 against 3249.4 for the raw card; ratio 0.944. Click-through (`WS_EX_LAYERED | WS_EX_TRANSPARENT`) still renders. No focus steal (`foreground_after` is the previous foreground), `WS_EX_TOOLWINDOW`/`WS_EX_NOACTIVATE`/topmost set, no taskbar entry.
- **wgpu:** a normal DXGI swapchain is opaque. `CompositeAlphaMode` came back as `Auto` and transparent pixels rendered black. Setting `Dx12SwapchainKind::DxgiFromVisual`, alpha mode `PreMultiplied` before the first configure, and `WS_EX_NOREDIRECTIONBITMAP` at window creation gives real per-pixel alpha (ratio 0.952). Without `NOREDIRECTIONBITMAP` the window is opaque. Removing `BringWindowToTop` fixed focus stealing; click-through works but needs the layered style.
- **Slint:** the femtovg window is layered and transparent (ratio 0.945). The text/card capture was intermittent in the harness, but the window path itself worked. Skia's transparent window produced inconsistent results and very high memory.

### Gate 2: DWM acrylic while unfocused

This gate failed for every candidate that also has per-pixel alpha.

- `DWMWA_SYSTEMBACKDROP_TYPE = DWMSBT_TRANSIENTWINDOW` plus `DwmExtendFrameIntoClientArea`: on a normal window the content is opaque; on a layered or `NOREDIRECTIONBITMAP` window the backdrop does not render. Widget variance 0-45 against 3320 for the test card, i.e. flat.
- `SetWindowCompositionAttribute` with `ACCENT_ENABLE_ACRYLICBLURBEHIND`: flat (variance 0, one colour) when unfocused.
- `DwmEnableBlurBehindWindow`: the window becomes transparent but the test card stays sharp. Pixel samples differ per 48 px square after the call, so this is a tint, not a blur.

DWM acrylic and a per-pixel-alpha composition window are mutually exclusive on this build. The product therefore implements the **Acrylic** type with the same CPU backdrop pipeline as Ice: wallpaper (or live capture) -> blur -> tint -> noise -> rounded mask. It is static, fully unfocused-safe, and cheap at widget size. This is a documented deviation from "real DWM acrylic"; the alternative was to give up true transparency or the RAM budget.

### Gate 3: text shaping and fallback

- **Slint femtovg:** Arabic `مرحبا` measures 35 px joined vs 63 px with ZWNJ-separated isolated forms. The Parley/fontique path used by Slint 1.18 reports Thai, Devanagari, CJK and emoji fallbacks.
- **Slint software:** Arabic joined 53 px, isolated produced no ink at all. Slint's own docs say software text is "limited to western scripts". Gate 3 fails.
- **cosmic-text (wgpu and software):** Arabic `rtl = true`, no `.notdef`, 9 glyph IDs differ between joined and isolated forms. Fallback families: `Segoe UI` (Arabic), `Leelawadee UI` (Thai), `Nirmala UI` (Devanagari), `Malgun Gothic` / `Microsoft YaHei UI` / `Yu Gothic` (CJK), `Segoe UI Emoji` (emoji). Emoji band saturation reaches 1.0, so colour glyphs are rendered. Gate 3 passes.

### Gate 4: runtime font loading and metric overrides

- Slint registers fonts at runtime through `fontique_011::shared_collection()`; `family_id("Lora")` goes from `None` to `Some` after registration.
- cosmic-text loads `.ttf` files with `FontSystem::db_mut().load_font_data()`. Both Georgia and Bebas Neue render from disk.
- Cap-height normalization at a 40 px base gives a multiplier of 0.9898 and a baseline offset of about -1.05 px. Rendered `H` cap heights: Georgia 28 px, normalized Bebas 29 px (1 px anti-aliasing difference). Gate 4 passes.
- Variable fonts: the Lora variable TTF registered but did not render at display sizes in the Slint harness. The software path renders static fonts. Ship static instances and reject variable fonts with a clear message.

### Gate 5: line animation without a continuous loop

The chosen renderer only redraws when the state changes: a 600 ms eased opacity/position transition runs at 60 fps, then the event loop goes back to `Wait`. Release CPU over the transition is 13.02% of one core (about 0.81% total); idle is 0-1.56% of one core (about 0.10% total). No timers or frames while hidden, paused, or idle.

### Gate 6: memory

The chosen software renderer is the only candidate under 40 MB: 18.1-21.0 MB working set and 3.5-3.7 MB private, idle and animating. The 1.87 MB release executable leaves a lot of headroom for the tray, IPC and background pipeline.

## Why not Slint

Three independent blockers:

1. Gate 3 fails in the software renderer, which is the only Slint renderer with acceptable memory.
2. The GPU renderers (femtovg, Skia) are 3-15x the memory budget and still fail gate 2.
3. Slint is `GPL-3.0-only OR Slint Royalty-free 2.0 OR Slint Software 3.0`. Iceberg is MIT; adding Slint would relicense the app or require a commercial-style license. That is not acceptable for this repository.

## Why not wgpu

wgpu is technically good: shaping, transparency with the composition path, click-through and focus all worked, and the executable is small. It fails only memory: 314-321 MB private on the only GPU available here. The widget would be the largest thing on the desktop by an order of magnitude. A software renderer with the same cosmic-text shaping pipeline is both smaller and simpler.

## Decision

Build the widget with:

- `winit` window, `WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE`, topmost, no taskbar entry.
- `UpdateLayeredWindow` with a top-down 32-bit premultiplied BGRA DIB.
- `cosmic-text` for layout, shaping, bidi and font fallback; `swash` (through cosmic-text) for glyph rasterization.
- CPU background pipeline: wallpaper read, crop/scale, separable blur at reduced resolution, tint, precomputed noise tile, rounded SDF mask. Acrylic, Ice and Liquid glass share it.
- Preset thumbnails rendered by the same code in the `xtask`; no HTML previews.
- `ttf-parser` for cap-height/x-height/ascender/descender metrics at import.

The Tauri settings app remains a separate on-demand process and never renders the widget. The widget process owns the tray and is the only always-on process.

## Consequences

- No DWM acrylic. Acrylic is a CPU-composited wallpaper blur. It will look like acrylic, survive unfocused, and cost no GPU memory. The settings copy should not claim "DWM" for it.
- Live backdrop capture (Windows.Graphics.Capture) is not part of v1. Wallpaper backdrop is the default and is processed once per move/resize/wallpaper change. If Live is added later, it must be opt-in with a visible battery warning and a 15 fps cap.
- Animations are CPU-rendered. At 300x120 this is cheap; the implementation will cache the card and glyph layers so a line transition only recomposites the text strip.
- The renderer is ~1.9 MB and the process is ~20 MB before the tray/IPC work. The Phase A/B budget is realistic.
- Windows 10, 150/200% DPI, two monitors and a 20-minute leak run still need to be verified in Phase E.
