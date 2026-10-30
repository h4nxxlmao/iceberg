use std::fs::File;
use std::path::Path;

use iceberg_core::config::{Config, WidgetMode};

use crate::demo;
use crate::dpi::Scale;
use crate::render::{Content, Renderer, View};

pub fn render_screenshots(config: &Config, directory: &Path) -> Result<(), String> {
    std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    let effective = config.effective();
    let mut renderer = Renderer::new();

    let card = demo::track();
    let card_lyrics = demo::lyrics();
    let card_playback = demo::playback();
    let synced = Content::Synced;
    for (name, scale) in [
        ("card-100.png", Scale::new(1.0, 100)),
        ("card-150.png", Scale::new(1.5, 100)),
        ("card-200.png", Scale::new(2.0, 100)),
    ] {
        let view = View {
            config: &effective,
            mode: WidgetMode::Card,
            track: Some(&card),
            playback: &card_playback,
            lyrics: Some(&card_lyrics),
            content: &synced,
            art: None,
            current_index: Some(2),
            transition: None,
            now_ms: 74_000,
            scale,
        };
        render_scene(&mut renderer, &view, &directory.join(name))?;
    }

    let view = View {
        config: &effective,
        mode: WidgetMode::Lyrics,
        track: Some(&card),
        playback: &card_playback,
        lyrics: Some(&card_lyrics),
        content: &synced,
        art: None,
        current_index: Some(2),
        transition: None,
        now_ms: 74_000,
        scale: Scale::new(1.0, 100),
    };
    render_scene(&mut renderer, &view, &directory.join("lyrics-100.png"))?;

    let view = View {
        config: &effective,
        mode: WidgetMode::Island,
        track: Some(&card),
        playback: &card_playback,
        lyrics: Some(&card_lyrics),
        content: &synced,
        art: None,
        current_index: Some(2),
        transition: None,
        now_ms: 74_000,
        scale: Scale::new(1.0, 100),
    };
    render_scene(&mut renderer, &view, &directory.join("island-100.png"))?;

    let empty_playback = iceberg_core::Playback::default();
    let empty = Content::Empty;
    let view = View {
        config: &effective,
        mode: WidgetMode::Card,
        track: None,
        playback: &empty_playback,
        lyrics: None,
        content: &empty,
        art: None,
        current_index: None,
        transition: None,
        now_ms: 0,
        scale: Scale::new(1.0, 100),
    };
    render_scene(&mut renderer, &view, &directory.join("state-empty-100.png"))?;

    let loading = Content::Loading;
    let view = View {
        config: &effective,
        mode: WidgetMode::Card,
        track: Some(&card),
        playback: &card_playback,
        lyrics: None,
        content: &loading,
        art: None,
        current_index: None,
        transition: None,
        now_ms: 74_000,
        scale: Scale::new(1.0, 100),
    };
    render_scene(
        &mut renderer,
        &view,
        &directory.join("state-loading-100.png"),
    )?;

    let no_lyrics = Content::NoLyrics;
    let view = View {
        config: &effective,
        mode: WidgetMode::Card,
        track: Some(&card),
        playback: &card_playback,
        lyrics: None,
        content: &no_lyrics,
        art: None,
        current_index: None,
        transition: None,
        now_ms: 74_000,
        scale: Scale::new(1.0, 100),
    };
    render_scene(
        &mut renderer,
        &view,
        &directory.join("state-nolyrics-100.png"),
    )?;

    let instrumental = Content::Instrumental;
    let view = View {
        config: &effective,
        mode: WidgetMode::Card,
        track: Some(&card),
        playback: &card_playback,
        lyrics: None,
        content: &instrumental,
        art: None,
        current_index: None,
        transition: None,
        now_ms: 74_000,
        scale: Scale::new(1.0, 100),
    };
    render_scene(
        &mut renderer,
        &view,
        &directory.join("state-instrumental-100.png"),
    )?;

    let plain_lyrics = demo::plain_lyrics();
    let plain = Content::Plain;
    let view = View {
        config: &effective,
        mode: WidgetMode::Card,
        track: Some(&card),
        playback: &card_playback,
        lyrics: Some(&plain_lyrics),
        content: &plain,
        art: None,
        current_index: None,
        transition: None,
        now_ms: 74_000,
        scale: Scale::new(1.0, 100),
    };
    render_scene(
        &mut renderer,
        &view,
        &directory.join("state-unsynced-100.png"),
    )?;

    let arabic_track = demo::arabic_track();
    let arabic_lyrics = demo::arabic_lyrics();
    let arabic_playback = demo::arabic_playback();
    let view = View {
        config: &effective,
        mode: WidgetMode::Card,
        track: Some(&arabic_track),
        playback: &arabic_playback,
        lyrics: Some(&arabic_lyrics),
        content: &synced,
        art: None,
        current_index: Some(1),
        transition: None,
        now_ms: 4_000,
        scale: Scale::new(1.0, 100),
    };
    render_scene(&mut renderer, &view, &directory.join("card-arabic-100.png"))?;

    std::fs::write(
        directory.join("README.md"),
        "# Phase A renderer previews\n\n\
         Generated by `cargo run -p iceberg-widget -- --screenshot=../docs/phase-a` from `app`.\n\n\
         These are offscreen renders over a neutral review backdrop, not a design sign-off.\n\
         The real widget is a transparent layered window on the desktop.\n\n\
         - `card-100.png`, `card-150.png`, `card-200.png`: Card mode at 100, 150 and 200% scale\n\
         - `lyrics-100.png`, `island-100.png`: the other two modes\n\
         - `state-*.png`: empty, loading, no lyrics, instrumental, unsynced\n\
         - `card-arabic-100.png`: Arabic shaping and RTL\n\
         - `bench-*.json`: release measurements at each scale\n",
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
