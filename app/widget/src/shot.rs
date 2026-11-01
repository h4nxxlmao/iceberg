use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use iceberg_core::config::{BackgroundKind, Config, WidgetMode};
use iceberg_core::fonts::{self, stock_font_dir, FontEntry, FontLibrary, FontMetrics, STOCK_FONTS};
use iceberg_core::model::{Lyrics, Playback, Track};
use iceberg_core::paths::Paths;
use iceberg_core::presets::builtin_presets;
use serde::Serialize;

use crate::backdrop::{BackdropCrop, Wallpaper};
use crate::demo;
use crate::dpi::Scale;
use crate::render::{Content, Renderer, View};

struct SceneFont {
    family: String,
    metrics: FontMetrics,
    entry: Option<Arc<FontEntry>>,
}

impl SceneFont {
    fn resolve(library: &mut FontLibrary, config: &Config, track: Option<&Track>) -> Self {
        let requested = track
            .map(|track| config.font_for(&track.key))
            .unwrap_or_else(|| config.typography.font_family.clone());
        let entry = library
            .resolve(&requested)
            .or_else(|| library.resolve("Lato"));
        Self {
            family: entry
                .as_ref()
                .map(|entry| entry.info.family.clone())
                .unwrap_or_else(|| "Lato".to_string()),
            metrics: entry
                .as_ref()
                .map(|entry| entry.metrics)
                .unwrap_or_default(),
            entry,
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn scene_view<'a>(
    config: &'a Config,
    font: &'a SceneFont,
    mode: WidgetMode,
    track: Option<&'a Track>,
    playback: &'a Playback,
    lyrics: Option<&'a Lyrics>,
    content: &'a Content,
    current_index: Option<usize>,
    now_ms: i64,
    scale: Scale,
) -> View<'a> {
    scene_view_with_backdrop(
        config,
        font,
        mode,
        track,
        playback,
        lyrics,
        content,
        current_index,
        now_ms,
        scale,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn scene_view_with_backdrop<'a>(
    config: &'a Config,
    font: &'a SceneFont,
    mode: WidgetMode,
    track: Option<&'a Track>,
    playback: &'a Playback,
    lyrics: Option<&'a Lyrics>,
    content: &'a Content,
    current_index: Option<usize>,
    now_ms: i64,
    scale: Scale,
    backdrop: Option<&'a BackdropCrop>,
) -> View<'a> {
    View {
        config,
        mode,
        track,
        playback,
        lyrics,
        content,
        art: None,
        current_index,
        transition: None,
        now_ms,
        scale,
        font_family: &font.family,
        font_metrics: font.metrics,
        font_entry: font.entry.as_deref(),
        backdrop,
    }
}

pub fn render_screenshots(config: &Config, directory: &Path) -> Result<(), String> {
    std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    let mut effective = config.effective();
    effective.background.kind = BackgroundKind::Solid;
    let paths = Paths::resolve();
    let mut library = FontLibrary::new(stock_font_dir(), paths.fonts);
    let mut renderer = Renderer::new();

    let card = demo::track();
    let card_lyrics = demo::lyrics();
    let card_playback = demo::playback();
    let synced = Content::Synced;
    let font = SceneFont::resolve(&mut library, &effective, Some(&card));

    for (name, scale) in [
        ("card-100.png", Scale::new(1.0, 100)),
        ("card-150.png", Scale::new(1.5, 100)),
        ("card-200.png", Scale::new(2.0, 100)),
    ] {
        let view = scene_view(
            &effective,
            &font,
            WidgetMode::Card,
            Some(&card),
            &card_playback,
            Some(&card_lyrics),
            &synced,
            Some(2),
            74_000,
            scale,
        );
        render_scene(&mut renderer, &view, &directory.join(name))?;
    }

    let view = scene_view(
        &effective,
        &font,
        WidgetMode::Lyrics,
        Some(&card),
        &card_playback,
        Some(&card_lyrics),
        &synced,
        Some(2),
        74_000,
        Scale::new(1.0, 100),
    );
    render_scene(&mut renderer, &view, &directory.join("lyrics-100.png"))?;

    let view = scene_view(
        &effective,
        &font,
        WidgetMode::Player,
        Some(&card),
        &card_playback,
        Some(&card_lyrics),
        &synced,
        Some(2),
        74_000,
        Scale::new(1.0, 100),
    );
    render_scene(&mut renderer, &view, &directory.join("player-100.png"))?;

    let view = scene_view(
        &effective,
        &font,
        WidgetMode::Island,
        Some(&card),
        &card_playback,
        Some(&card_lyrics),
        &synced,
        Some(2),
        74_000,
        Scale::new(1.0, 100),
    );
    render_scene(&mut renderer, &view, &directory.join("island-100.png"))?;

    let empty_playback = iceberg_core::Playback::default();
    let empty = Content::Empty;
    let view = scene_view(
        &effective,
        &font,
        WidgetMode::Card,
        None,
        &empty_playback,
        None,
        &empty,
        None,
        0,
        Scale::new(1.0, 100),
    );
    render_scene(&mut renderer, &view, &directory.join("state-empty-100.png"))?;

    let loading = Content::Loading;
    let view = scene_view(
        &effective,
        &font,
        WidgetMode::Card,
        Some(&card),
        &card_playback,
        None,
        &loading,
        None,
        74_000,
        Scale::new(1.0, 100),
    );
    render_scene(
        &mut renderer,
        &view,
        &directory.join("state-loading-100.png"),
    )?;

    let no_lyrics = Content::NoLyrics;
    let view = scene_view(
        &effective,
        &font,
        WidgetMode::Card,
        Some(&card),
        &card_playback,
        None,
        &no_lyrics,
        None,
        74_000,
        Scale::new(1.0, 100),
    );
    render_scene(
        &mut renderer,
        &view,
        &directory.join("state-nolyrics-100.png"),
    )?;

    let instrumental = Content::Instrumental;
    let view = scene_view(
        &effective,
        &font,
        WidgetMode::Card,
        Some(&card),
        &card_playback,
        None,
        &instrumental,
        None,
        74_000,
        Scale::new(1.0, 100),
    );
    render_scene(
        &mut renderer,
        &view,
        &directory.join("state-instrumental-100.png"),
    )?;

    let plain_lyrics = demo::plain_lyrics();
    let plain = Content::Plain;
    let view = scene_view(
        &effective,
        &font,
        WidgetMode::Card,
        Some(&card),
        &card_playback,
        Some(&plain_lyrics),
        &plain,
        None,
        74_000,
        Scale::new(1.0, 100),
    );
    render_scene(
        &mut renderer,
        &view,
        &directory.join("state-unsynced-100.png"),
    )?;

    let arabic_track = demo::arabic_track();
    let arabic_lyrics = demo::arabic_lyrics();
    let arabic_playback = demo::arabic_playback();
    let view = scene_view(
        &effective,
        &font,
        WidgetMode::Card,
        Some(&arabic_track),
        &arabic_playback,
        Some(&arabic_lyrics),
        &synced,
        Some(1),
        4_000,
        Scale::new(1.0, 100),
    );
    render_scene(&mut renderer, &view, &directory.join("card-arabic-100.png"))?;

    for stock in STOCK_FONTS {
        let mut preview = effective.clone();
        preview.per_song.clear();
        preview.typography.font_family = stock.family.to_string();
        let font = SceneFont::resolve(&mut library, &preview, Some(&card));
        let view = scene_view(
            &preview,
            &font,
            WidgetMode::Card,
            Some(&card),
            &card_playback,
            Some(&card_lyrics),
            &synced,
            Some(2),
            74_000,
            Scale::new(1.0, 100),
        );
        render_scene(
            &mut renderer,
            &view,
            &directory.join(format!("font-{}-100.png", stock.id)),
        )?;
    }

    let wallpaper = synthetic_wallpaper();
    let crop = wallpaper.crop_cover([0, 0, 1920, 1080], [760, 418, 388, 245], 388, 245);
    let presets = builtin_presets();
    for (name, preset_id, kind, chromatic) in [
        ("background-solid-100.png", "ink", None, false),
        ("background-transparent-100.png", "clear", None, false),
        (
            "background-acrylic-100.png",
            "ink",
            Some(BackgroundKind::Acrylic),
            false,
        ),
        ("background-ice-100.png", "frost", None, false),
        ("background-liquid-100.png", "glass", None, false),
        ("background-liquid-chromatic-100.png", "glass", None, true),
    ] {
        let mut scene = effective.clone();
        if let Some(preset) = presets.iter().find(|preset| preset.id == preset_id) {
            preset.apply_to(&mut scene);
        }
        if let Some(kind) = kind {
            scene.background.kind = kind;
        }
        if chromatic {
            scene.background.liquid_chromatic = true;
        }
        let scene_font = SceneFont::resolve(&mut library, &scene, Some(&card));
        let view = scene_view_with_backdrop(
            &scene,
            &scene_font,
            WidgetMode::Card,
            Some(&card),
            &card_playback,
            Some(&card_lyrics),
            &synced,
            Some(2),
            74_000,
            Scale::new(1.0, 100),
            Some(&crop),
        );
        render_scene(&mut renderer, &view, &directory.join(name))?;
    }

    std::fs::write(
        directory.join("README.md"),
        "# Native renderer previews\n\n\
         Generated by `cargo run -p iceberg-widget -- --screenshot=../docs/phase-c` from `app`.\n\n\
         These are offscreen renders over a neutral review backdrop, not a design sign-off.\n\
         The real widget is a transparent layered window on the desktop.\n\n\
         - `card-100.png`, `card-150.png`, `card-200.png`: Card mode at 100, 150 and 200% scale\n\
         - `lyrics-100.png`, `island-100.png`: the other two modes\n\
         - `state-*.png`: empty, loading, no lyrics, instrumental, unsynced\n\
         - `card-arabic-100.png`: Arabic shaping and RTL with the normal system fallback\n\
         - `font-*-100.png`: the same Card at 100% with each stock family, using normalized metrics\n\
         - `background-*.png`: Solid, Transparent, Acrylic, Ice, Liquid, and Liquid with chromatic refraction\n\
         - `bench-*.json`: release measurements at each scale\n\n\
         The background previews use a generated review wallpaper, not the desktop wallpaper.\n\
         The other previews use a solid card so typography is comparable. The typography\n\
         Card previews stay 388 x 245 at 100%; the transparent surface uses the Clear preset's\n\
         no-shadow 360 x 212 geometry. Optical size and baseline are normalized, while line\n\
         spacing stays on the user's line-height grid. See `RESULTS.md` for numbers.\n\n\
         Stock families: Lato, Poppins, Abril Fatface, Barlow Condensed, Amatic SC,\n\
         Space Mono, Varela Round, Lobster. All are OFL 1.1; licenses are in\n\
         `app/assets/fonts/<family>/OFL.txt`.\n",
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

pub fn render_settings_assets(directory: &Path) -> Result<(), String> {
    let presets_dir = directory.join("presets");
    let surfaces_dir = directory.join("surfaces");
    std::fs::create_dir_all(&presets_dir).map_err(|error| error.to_string())?;
    std::fs::create_dir_all(&surfaces_dir).map_err(|error| error.to_string())?;
    let paths = Paths::resolve();
    let mut library = FontLibrary::new(stock_font_dir(), paths.fonts);
    let mut renderer = Renderer::new();
    let card = demo::track();
    let lyrics = demo::lyrics();
    let playback = demo::playback();
    let synced = Content::Synced;
    let wallpaper = synthetic_wallpaper();
    let crop = wallpaper.crop_cover([0, 0, 1920, 1080], [760, 418, 388, 245], 388, 245);

    for preset in builtin_presets() {
        let mut config = Config::default();
        preset.apply_to(&mut config);
        config.per_song.clear();
        let font = SceneFont::resolve(&mut library, &config, Some(&card));
        let view = scene_view_with_backdrop(
            &config,
            &font,
            WidgetMode::Card,
            Some(&card),
            &playback,
            Some(&lyrics),
            &synced,
            Some(2),
            74_000,
            Scale::new(1.0, 100),
            Some(&crop),
        );
        render_scene(
            &mut renderer,
            &view,
            &presets_dir.join(format!("{}.png", preset.id)),
        )?;
    }

    for (kind, preset_id) in [
        (BackgroundKind::Solid, "ink"),
        (BackgroundKind::Transparent, "clear"),
        (BackgroundKind::Acrylic, "ink"),
        (BackgroundKind::Ice, "frost"),
        (BackgroundKind::Liquid, "glass"),
    ] {
        let mut config = Config::default();
        if let Some(preset) = builtin_presets()
            .into_iter()
            .find(|preset| preset.id == preset_id)
        {
            preset.apply_to(&mut config);
        }
        config.background.kind = kind;
        config.per_song.clear();
        let font = SceneFont::resolve(&mut library, &config, Some(&card));
        let view = scene_view_with_backdrop(
            &config,
            &font,
            WidgetMode::Card,
            Some(&card),
            &playback,
            Some(&lyrics),
            &synced,
            Some(2),
            74_000,
            Scale::new(1.0, 100),
            Some(&crop),
        );
        render_scene(
            &mut renderer,
            &view,
            &surfaces_dir.join(format!("{}.png", background_filename(kind))),
        )?;
    }
    Ok(())
}

fn background_filename(kind: BackgroundKind) -> &'static str {
    match kind {
        BackgroundKind::Solid => "solid",
        BackgroundKind::Transparent => "transparent",
        BackgroundKind::Acrylic => "acrylic",
        BackgroundKind::Ice => "ice",
        BackgroundKind::Liquid => "liquid",
    }
}

#[derive(Serialize)]
struct FontPreviewEntry {
    family: String,
    file: String,
}

pub fn render_font_previews(directory: &Path, text: &str) -> Result<(), String> {
    std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    let paths = Paths::resolve();
    let user_dir = paths.fonts.clone();
    let mut library = FontLibrary::new(stock_font_dir(), paths.fonts);
    let mut renderer = Renderer::new();
    let line: String = text
        .trim()
        .replace(['\r', '\n'], " ")
        .chars()
        .take(120)
        .collect();
    let line = if line.is_empty() {
        "The night city grows".to_string()
    } else {
        line
    };
    let mut manifest = Vec::new();
    for (index, font) in fonts::catalog(&user_dir).iter().enumerate() {
        let Some(entry) = library.resolve(&font.family) else {
            continue;
        };
        let Some(png) = renderer.font_sample_png(&entry, &line, 320, 180) else {
            continue;
        };
        let file = format!("font-{index}.png");
        std::fs::write(directory.join(&file), png).map_err(|error| error.to_string())?;
        manifest.push(FontPreviewEntry {
            family: font.family.clone(),
            file,
        });
    }
    let json = serde_json::to_vec(&manifest).map_err(|error| error.to_string())?;
    std::fs::write(directory.join("previews.json"), json).map_err(|error| error.to_string())?;
    Ok(())
}

fn synthetic_wallpaper() -> Wallpaper {
    const WIDTH: usize = 960;
    const HEIGHT: usize = 540;
    let mut rgba = vec![0u8; WIDTH * HEIGHT * 4];
    for y in 0..HEIGHT {
        let fy = y as f32 / (HEIGHT - 1) as f32;
        for x in 0..WIDTH {
            let fx = x as f32 / (WIDTH - 1) as f32;
            let mut r = 9.0 + 14.0 * fx + 8.0 * fy;
            let mut g = 14.0 + 22.0 * fx + 12.0 * fy;
            let mut b = 17.0 + 26.0 * fx + 16.0 * fy;

            let dx = (fx - 0.33) / 0.32;
            let dy = (fy - 0.44) / 0.30;
            let teal = (1.0 - (dx * dx + dy * dy)).max(0.0).powi(2);
            r += teal * 14.0;
            g += teal * 58.0;
            b += teal * 64.0;

            let warm = (1.0 - ((fx - 0.535).abs() / 0.05)).clamp(0.0, 1.0).powi(2)
                * (1.0 - ((fy - 0.5).abs() / 0.46)).clamp(0.0, 1.0);
            r += warm * 200.0;
            g += warm * 66.0;
            b += warm * 32.0;

            let pale = (1.0 - ((fx * 0.65 + fy * 0.35 - 0.45).abs() / 0.055))
                .clamp(0.0, 1.0)
                .powi(2)
                * 0.5;
            r += pale * 160.0;
            g += pale * 150.0;
            b += pale * 130.0;

            let grain = (synthetic_noise(x as u32, y as u32) - 0.5) * 8.0;
            let index = (y * WIDTH + x) * 4;
            rgba[index] = (r + grain).clamp(0.0, 255.0) as u8;
            rgba[index + 1] = (g + grain).clamp(0.0, 255.0) as u8;
            rgba[index + 2] = (b + grain).clamp(0.0, 255.0) as u8;
            rgba[index + 3] = 255;
        }
    }
    Wallpaper::from_rgba(WIDTH, HEIGHT, rgba).expect("synthetic wallpaper")
}

fn synthetic_noise(x: u32, y: u32) -> f32 {
    let mut value = x
        .wrapping_mul(0x9e37_79b1)
        .wrapping_add(y.wrapping_mul(0x85eb_ca77))
        .wrapping_add(0x51ed_270b);
    value ^= value >> 16;
    value = value.wrapping_mul(0x7feb_352d);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846c_a68b);
    value ^= value >> 16;
    (value & 0xffff) as f32 / 65_535.0
}

fn render_scene(renderer: &mut Renderer, view: &View<'_>, path: &Path) -> Result<(), String> {
    renderer.ensure_size(view);
    renderer.clear_text_cache();
    renderer.rebuild_background(view);
    renderer.render(view);
    let metrics = view.metrics();
    let preview = preview_frame(renderer.frame(), metrics.window_w, metrics.window_h);
    write_png(path, metrics.window_w, metrics.window_h, &preview)
}

fn preview_frame(frame: &[u8], width: i32, height: i32) -> Vec<u8> {
    let mut rgba = vec![0u8; (width * height * 4) as usize];
    for y in 0..height {
        let t = y as f32 / height.max(1) as f32;
        let background = [
            (30.0 + (18.0 - 30.0) * t) as u8,
            (33.0 + (21.0 - 33.0) * t) as u8,
            (36.0 + (24.0 - 36.0) * t) as u8,
        ];
        for x in 0..width {
            let index = ((y * width + x) * 4) as usize;
            let alpha = frame[index + 3] as u32;
            let inverse = 255 - alpha;
            rgba[index] =
                (frame[index + 2] as u32 + background[0] as u32 * inverse / 255).min(255) as u8;
            rgba[index + 1] =
                (frame[index + 1] as u32 + background[1] as u32 * inverse / 255).min(255) as u8;
            rgba[index + 2] =
                (frame[index] as u32 + background[2] as u32 * inverse / 255).min(255) as u8;
            rgba[index + 3] = 255;
        }
    }
    rgba
}

fn write_png(path: &Path, width: i32, height: i32, rgba: &[u8]) -> Result<(), String> {
    let file = File::create(path).map_err(|error| error.to_string())?;
    let mut encoder = png::Encoder::new(file, width as u32, height as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|error| error.to_string())?;
    writer
        .write_image_data(rgba)
        .map_err(|error| error.to_string())?;
    Ok(())
}
