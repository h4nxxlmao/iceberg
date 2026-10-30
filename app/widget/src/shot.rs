use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use iceberg_core::config::{Config, WidgetMode};
use iceberg_core::fonts::{stock_font_dir, FontEntry, FontLibrary, FontMetrics, STOCK_FONTS};
use iceberg_core::model::{Lyrics, Playback, Track};
use iceberg_core::paths::Paths;

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
    }
}

pub fn render_screenshots(config: &Config, directory: &Path) -> Result<(), String> {
    std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    let effective = config.effective();
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

    std::fs::write(
        directory.join("README.md"),
        "# Phase B typography previews\n\n\
         Generated by `cargo run -p iceberg-widget -- --screenshot=../docs/phase-b` from `app`.\n\n\
         These are offscreen renders over a neutral review backdrop, not a design sign-off.\n\
         The real widget is a transparent layered window on the desktop.\n\n\
         - `card-100.png`, `card-150.png`, `card-200.png`: Card mode at 100, 150 and 200% scale\n\
         - `lyrics-100.png`, `island-100.png`: the other two modes\n\
         - `state-*.png`: empty, loading, no lyrics, instrumental, unsynced\n\
         - `card-arabic-100.png`: Arabic shaping and RTL with the normal system fallback\n\
         - `font-*-100.png`: the same Card at 100% with each stock family, using normalized metrics\n\
         - `bench-*.json`: release measurements at each scale\n\n\
         Every Card preview stays 388 x 245 at 100%: optical size and baseline are normalized,\n\
         while line spacing stays on the user's line-height grid. See `RESULTS.md` for numbers.\n\n\
         Stock families: Lato, Poppins, Abril Fatface, Barlow Condensed, Amatic SC,\n\
         Space Mono, Varela Round, Lobster. All are OFL 1.1; licenses are in\n\
         `app/assets/fonts/<family>/OFL.txt`.\n",
    )
    .map_err(|error| error.to_string())?;
    Ok(())
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
