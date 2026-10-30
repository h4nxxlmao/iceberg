use std::collections::HashMap;

use cosmic_text::{
    Attrs, Buffer, Color, Family, FontSystem, Metrics as TextMetrics, Shaping, Style, SwashCache,
    Weight, Wrap,
};
use iceberg_core::config::{Align, BackgroundKind, Config, TextCase, WidgetMode};
use iceberg_core::model::{Lyrics, Playback, Track};

use crate::dpi::Scale;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    #[cfg(test)]
    pub fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub fn from_hex(value: &str, alpha: f64) -> Self {
        let clean = value.trim().trim_start_matches('#');
        let (r, g, b) = match clean.len() {
            3 => {
                let r = u8::from_str_radix(&clean[0..1].repeat(2), 16).unwrap_or(0x0b);
                let g = u8::from_str_radix(&clean[1..2].repeat(2), 16).unwrap_or(0x0f);
                let b = u8::from_str_radix(&clean[2..3].repeat(2), 16).unwrap_or(0x11);
                (r, g, b)
            }
            6 => {
                let r = u8::from_str_radix(&clean[0..2], 16).unwrap_or(0x0b);
                let g = u8::from_str_radix(&clean[2..4], 16).unwrap_or(0x0f);
                let b = u8::from_str_radix(&clean[4..6], 16).unwrap_or(0x11);
                (r, g, b)
            }
            _ => (0x0b, 0x0f, 0x11),
        };
        Self {
            r,
            g,
            b,
            a: (alpha.clamp(0.0, 1.0) * 255.0).round() as u8,
        }
    }

    pub fn with_alpha(self, alpha: f64) -> Self {
        Self {
            a: (alpha.clamp(0.0, 1.0) * 255.0).round() as u8,
            ..self
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Transition {
    pub from: f32,
    pub to: f32,
    pub started_ms: i64,
    pub duration_ms: u32,
}

impl Transition {
    pub fn position(&self, now_ms: i64) -> f32 {
        if self.duration_ms == 0 {
            return self.to;
        }
        let elapsed = (now_ms - self.started_ms).max(0) as f32;
        let t = (elapsed / self.duration_ms as f32).clamp(0.0, 1.0);
        let eased = 1.0 - (1.0 - t).powi(3);
        self.from + (self.to - self.from) * eased
    }

    pub fn finished(&self, now_ms: i64) -> bool {
        now_ms - self.started_ms >= self.duration_ms as i64
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Content {
    Empty,
    Loading,
    Synced,
    Plain,
    Instrumental,
    NoLyrics,
    Error(String),
}

#[derive(Clone, Debug)]
pub struct ArtImage {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

impl ArtImage {
    pub fn from_png(bytes: &[u8], target: usize) -> Option<Self> {
        if target == 0 {
            return None;
        }
        let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
        let mut reader = decoder.read_info().ok()?;
        let mut buffer = vec![0u8; reader.output_buffer_size()?];
        let info = reader.next_frame(&mut buffer).ok()?;
        if info.bit_depth != png::BitDepth::Eight {
            return None;
        }
        let pixels = &buffer[..info.buffer_size()];
        let source_width = info.width as usize;
        let source_height = info.height as usize;
        if source_width == 0 || source_height == 0 {
            return None;
        }
        let mut rgba = Vec::with_capacity(source_width * source_height * 4);
        match info.color_type {
            png::ColorType::Rgba => rgba.extend_from_slice(pixels),
            png::ColorType::Rgb => {
                for pixel in pixels.chunks_exact(3) {
                    rgba.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]);
                }
            }
            png::ColorType::Grayscale => {
                for value in pixels {
                    rgba.extend_from_slice(&[*value, *value, *value, 255]);
                }
            }
            png::ColorType::GrayscaleAlpha => {
                for pixel in pixels.chunks_exact(2) {
                    rgba.extend_from_slice(&[pixel[0], pixel[0], pixel[0], pixel[1]]);
                }
            }
            png::ColorType::Indexed => return None,
        }
        if rgba.len() < source_width * source_height * 4 {
            return None;
        }
        let mut scaled = vec![0u8; target * target * 4];
        for y in 0..target {
            let sy0 = y * source_height / target;
            let sy1 = ((y + 1) * source_height / target).max(sy0 + 1);
            for x in 0..target {
                let sx0 = x * source_width / target;
                let sx1 = ((x + 1) * source_width / target).max(sx0 + 1);
                let mut sum = [0u32; 4];
                let mut count = 0u32;
                for sy in sy0..sy1.min(source_height) {
                    for sx in sx0..sx1.min(source_width) {
                        let index = (sy * source_width + sx) * 4;
                        for channel in 0..4 {
                            sum[channel] += rgba[index + channel] as u32;
                        }
                        count += 1;
                    }
                }
                let index = (y * target + x) * 4;
                let divisor = count.max(1);
                for channel in 0..4 {
                    scaled[index + channel] = (sum[channel] / divisor) as u8;
                }
            }
        }
        Some(Self {
            width: target,
            height: target,
            rgba: scaled,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Metrics {
    pub scale: f64,
    pub window_w: i32,
    pub window_h: i32,
    pub margin: f64,
    pub card_x: f64,
    pub card_y: f64,
    pub card_w: f64,
    pub card_h: f64,
    pub padding: f64,
    pub art_x: f64,
    pub art_y: f64,
    pub art_size: f64,
    pub meta_x: f64,
    pub meta_y: f64,
    pub meta_w: f64,
    pub progress_x: f64,
    pub progress_y: f64,
    pub progress_w: f64,
    pub progress_h: f64,
    pub lyric_x: f64,
    pub lyric_y: f64,
    pub lyric_w: f64,
    pub font_size: f64,
    pub line_height: f64,
    pub visible_lines: usize,
    pub radius: f64,
    pub border: f64,
}

pub fn compute_metrics(config: &Config, mode: WidgetMode, scale: Scale) -> Metrics {
    let effective = scale.effective();
    let padding = config.background.padding * effective;
    let font_size = config.typography.font_size * effective;
    let line_height = font_size * config.typography.line_height;
    let visible_lines = config.typography.visible_lines as usize;
    let card_w = config.layout.width * effective;
    let margin = if config.background.shadow {
        14.0 * effective
    } else {
        0.0
    };
    let show_progress = config.behavior.show_progress
        && mode == WidgetMode::Card
        && config.background.kind != BackgroundKind::Transparent;
    let gap = 10.0 * effective;
    let progress_h = 2.0 * effective;

    let (art_size, _meta_h, progress_y, lyric_y, lyric_lines, card_h) = match mode {
        WidgetMode::Card => {
            let art_size = 44.0 * effective;
            let meta_h = art_size.max(font_size * 2.2);
            let progress_y = padding + meta_h + gap;
            let lyric_y = progress_y + if show_progress { progress_h + gap } else { 0.0 };
            let card_h = lyric_y + visible_lines as f64 * line_height + padding;
            (art_size, meta_h, progress_y, lyric_y, visible_lines, card_h)
        }
        WidgetMode::Lyrics => {
            let card_h = padding * 2.0 + visible_lines as f64 * line_height;
            (0.0, 0.0, 0.0, padding, visible_lines, card_h)
        }
        WidgetMode::Island => {
            let art_size = 28.0 * effective;
            let meta_h = art_size.max(font_size * 1.95);
            let lyric_y = padding + meta_h + 4.0 * effective;
            let card_h = lyric_y + line_height + padding;
            (art_size, meta_h, 0.0, lyric_y, 1, card_h)
        }
    };

    let card_x = margin;
    let card_y = margin;
    let content_x = card_x + padding;
    let art_x = content_x;
    let art_y = card_y + padding;
    let meta_x = art_x
        + art_size
        + if art_size > 0.0 {
            12.0 * effective
        } else {
            0.0
        };
    let meta_y = card_y + padding;
    let meta_w = (card_x + card_w - padding - meta_x).max(1.0);
    let lyric_x = content_x;
    let lyric_y_abs = card_y + lyric_y;
    let lyric_w = (card_w - padding * 2.0).max(1.0);
    let progress_x = content_x;
    let progress_y_abs = card_y + progress_y;
    let progress_w = lyric_w;
    let radius = (config.background.radius as f64 * effective)
        .min(card_w.min(card_h) / 2.0)
        .max(0.0);
    let border = config.background.border_width * effective;

    Metrics {
        scale: effective,
        window_w: (card_w + margin * 2.0).round() as i32,
        window_h: (card_h + margin * 2.0).round() as i32,
        margin,
        card_x,
        card_y,
        card_w,
        card_h,
        padding,
        art_x,
        art_y,
        art_size,
        meta_x,
        meta_y,
        meta_w,
        progress_x,
        progress_y: progress_y_abs,
        progress_w,
        progress_h,
        lyric_x,
        lyric_y: lyric_y_abs,
        lyric_w,
        font_size,
        line_height,
        visible_lines: lyric_lines,
        radius,
        border,
    }
}

#[derive(Clone, Copy)]
pub struct View<'a> {
    pub config: &'a Config,
    pub mode: WidgetMode,
    pub track: Option<&'a Track>,
    pub playback: &'a Playback,
    pub lyrics: Option<&'a Lyrics>,
    pub content: &'a Content,
    pub art: Option<&'a ArtImage>,
    pub current_index: Option<usize>,
    pub transition: Option<Transition>,
    pub now_ms: i64,
    pub scale: Scale,
}

impl<'a> View<'a> {
    pub fn metrics(&self) -> Metrics {
        compute_metrics(self.config, self.mode, self.scale)
    }

    fn accent(&self) -> Rgba {
        if self.config.background.accent_from_art {
            if let Some(accent) = self.track.and_then(|track| track.accent.as_deref()) {
                return Rgba::from_hex(accent, 1.0);
            }
        }
        Rgba::from_hex(&self.config.background.accent_color, 1.0)
    }

    fn active_color(&self) -> Rgba {
        Rgba::from_hex(&self.config.typography.active_color, 1.0)
    }

    fn font_family(&self) -> String {
        self.track
            .map(|track| self.config.font_for(&track.key))
            .unwrap_or_else(|| self.config.typography.font_family.clone())
    }
}

#[derive(Clone, Debug)]
struct TextMask {
    width: usize,
    height: usize,
    offset_x: i32,
    offset_y: i32,
    pixels: Vec<u8>,
}

pub struct Renderer {
    font_system: FontSystem,
    swash_cache: SwashCache,
    pub metrics: Option<Metrics>,
    background: Vec<u8>,
    text: Vec<u8>,
    frame: Vec<u8>,
    line_masks: HashMap<usize, TextMask>,
    plain_masks: HashMap<usize, TextMask>,
    state_masks: HashMap<String, TextMask>,
    pub background_rebuilds: u64,
}

impl Renderer {
    pub fn new() -> Self {
        Self {
            font_system: FontSystem::new(),
            swash_cache: SwashCache::new(),
            metrics: None,
            background: Vec::new(),
            text: Vec::new(),
            frame: Vec::new(),
            line_masks: HashMap::new(),
            plain_masks: HashMap::new(),
            state_masks: HashMap::new(),
            background_rebuilds: 0,
        }
    }

    pub fn ensure_size(&mut self, view: &View<'_>) -> bool {
        let metrics = view.metrics();
        if self.metrics.as_ref() == Some(&metrics) {
            return false;
        }
        self.metrics = Some(metrics.clone());
        let bytes = (metrics.window_w.max(1) * metrics.window_h.max(1) * 4) as usize;
        self.background = vec![0u8; bytes];
        self.text = vec![0u8; bytes];
        self.frame = vec![0u8; bytes];
        self.clear_text_cache();
        true
    }

    pub fn clear_text_cache(&mut self) {
        self.line_masks.clear();
        self.plain_masks.clear();
        self.state_masks.clear();
        self.swash_cache = SwashCache::new();
    }

    pub fn frame(&self) -> &[u8] {
        &self.frame
    }

    pub fn rebuild_background(&mut self, view: &View<'_>) {
        let Some(metrics) = self.metrics.clone() else {
            return;
        };
        self.background.fill(0);
        self.background_rebuilds = self.background_rebuilds.saturating_add(1);

        let active = view.active_color();
        if view.config.background.shadow {
            draw_shadow(&mut self.background, &metrics, view.config);
        }
        let fill = background_fill(view.config);
        fill_rounded_rect(
            &mut self.background,
            metrics.window_w,
            metrics.window_h,
            metrics.card_x,
            metrics.card_y,
            metrics.card_w,
            metrics.card_h,
            metrics.radius,
            fill,
        );
        if metrics.border > 0.0 && view.config.background.border_opacity > 0.0 {
            let border = Rgba::from_hex(
                &view.config.background.border_color,
                view.config.background.border_opacity,
            );
            stroke_rounded_rect(
                &mut self.background,
                metrics.window_w,
                metrics.window_h,
                metrics.card_x,
                metrics.card_y,
                metrics.card_w,
                metrics.card_h,
                metrics.radius,
                metrics.border,
                border,
            );
        }
        if metrics.art_size > 0.0 {
            self.draw_art(view, &metrics, active);
        }

        if metrics.meta_w > 1.0 && metrics.art_size > 0.0 {
            let title = view
                .track
                .map(|track| track.title.as_str())
                .unwrap_or("Iceberg");
            let artist = view
                .track
                .map(|track| track.artist.as_str())
                .unwrap_or("Nothing playing");
            let title_size = 13.5 * metrics.scale as f32;
            let artist_size = 11.5 * metrics.scale as f32;
            let family = view.font_family();
            let title_mask = self.rasterize_text(
                title,
                &family,
                title_size,
                600,
                false,
                0.0,
                active,
                metrics.meta_w as f32,
                1,
            );
            let title_x = aligned_x(
                view.config.typography.align,
                metrics.meta_x,
                metrics.meta_w,
                &title_mask,
            );
            blit_mask(
                &mut self.background,
                metrics.window_w,
                metrics.window_h,
                &title_mask,
                title_x,
                metrics.meta_y as i32,
                1.0,
                view.config.typography.shadow,
                false,
            );
            let artist_mask = self.rasterize_text(
                artist,
                &family,
                artist_size,
                400,
                false,
                0.0,
                active.with_alpha(0.54),
                metrics.meta_w as f32,
                1,
            );
            let artist_x = aligned_x(
                view.config.typography.align,
                metrics.meta_x,
                metrics.meta_w,
                &artist_mask,
            );
            blit_mask(
                &mut self.background,
                metrics.window_w,
                metrics.window_h,
                &artist_mask,
                artist_x,
                (metrics.meta_y + title_size as f64 * 1.42) as i32,
                1.0,
                view.config.typography.shadow,
                false,
            );
        }

        if view.config.behavior.show_progress
            && view.mode == WidgetMode::Card
            && view.config.background.kind != BackgroundKind::Transparent
        {
            let track = active.with_alpha(0.14);
            fill_rounded_rect(
                &mut self.background,
                metrics.window_w,
                metrics.window_h,
                metrics.progress_x,
                metrics.progress_y,
                metrics.progress_w,
                metrics.progress_h,
                metrics.progress_h / 2.0,
                track,
            );
        }
    }

    fn draw_art(&mut self, view: &View<'_>, metrics: &Metrics, active: Rgba) {
        let x = metrics.art_x;
        let y = metrics.art_y;
        let size = metrics.art_size;
        let radius = 3.0 * metrics.scale;
        if let Some(art) = view.art {
            let scale = art.width as f64 / size;
            for py in 0..size as i32 {
                for px in 0..size as i32 {
                    let dx = (px as f64 + 0.5 - size / 2.0).abs() - (size / 2.0 - radius);
                    let dy = (py as f64 + 0.5 - size / 2.0).abs() - (size / 2.0 - radius);
                    let distance = (dx.max(0.0).powi(2) + dy.max(0.0).powi(2)).sqrt()
                        + dx.max(dy).min(0.0)
                        - radius;
                    let coverage = (1.0 - distance).clamp(0.0, 1.0);
                    if coverage <= 0.0 {
                        continue;
                    }
                    let sx = ((px as f64 * scale) as usize).min(art.width.saturating_sub(1));
                    let sy = ((py as f64 * scale) as usize).min(art.height.saturating_sub(1));
                    let source = (sy * art.width + sx) * 4;
                    let alpha = (art.rgba[source + 3] as f64 * coverage) as u32;
                    let inverse = 255 - alpha;
                    let tx = x as i32 + px;
                    let ty = y as i32 + py;
                    if tx < 0 || ty < 0 || tx >= metrics.window_w || ty >= metrics.window_h {
                        continue;
                    }
                    let index = ((ty * metrics.window_w + tx) * 4) as usize;
                    let sr = art.rgba[source] as u32 * alpha / 255;
                    let sg = art.rgba[source + 1] as u32 * alpha / 255;
                    let sb = art.rgba[source + 2] as u32 * alpha / 255;
                    self.background[index] =
                        (sb + self.background[index] as u32 * inverse / 255).min(255) as u8;
                    self.background[index + 1] =
                        (sg + self.background[index + 1] as u32 * inverse / 255).min(255) as u8;
                    self.background[index + 2] =
                        (sr + self.background[index + 2] as u32 * inverse / 255).min(255) as u8;
                    self.background[index + 3] =
                        (alpha + self.background[index + 3] as u32 * inverse / 255).min(255) as u8;
                }
            }
        } else {
            fill_rounded_rect(
                &mut self.background,
                metrics.window_w,
                metrics.window_h,
                x,
                y,
                size,
                size,
                radius,
                Rgba::from_hex("#ffffff", 0.06),
            );
            let letter = view
                .track
                .map(|track| track.title.chars().next().unwrap_or('I'))
                .unwrap_or('I')
                .to_uppercase()
                .to_string();
            let family = view.font_family();
            let mask = self.rasterize_text(
                &letter,
                &family,
                (size * 0.42) as f32,
                600,
                false,
                0.0,
                active.with_alpha(0.55),
                size as f32,
                1,
            );
            let mask_x = x + (size - mask.width as f64) / 2.0 - mask.offset_x as f64;
            let mask_y = y + (size - mask.height as f64) / 2.0 - mask.offset_y as f64;
            blit_mask(
                &mut self.background,
                metrics.window_w,
                metrics.window_h,
                &mask,
                mask_x as i32,
                mask_y as i32,
                1.0,
                false,
                false,
            );
        }
    }

    pub fn render(&mut self, view: &View<'_>) {
        let Some(metrics) = self.metrics.clone() else {
            return;
        };
        self.text.fill(0);
        self.draw_content(view, &metrics);
        self.frame.copy_from_slice(&self.background);
        self.draw_progress(view, &metrics);
        self.blend_text(&metrics);
    }

    fn draw_content(&mut self, view: &View<'_>, metrics: &Metrics) {
        match view.content {
            Content::Synced => {
                let Some(lyrics) = view.lyrics else {
                    return;
                };
                let count = lyrics.lines.len();
                if count == 0 {
                    return;
                }
                let center = view
                    .transition
                    .map(|transition| transition.position(view.now_ms))
                    .or_else(|| view.current_index.map(|index| index as f32))
                    .unwrap_or(0.0);
                let lines = metrics.visible_lines.max(1);
                let before = (lines / 2) as f32;
                let range = visible_range(count, center, lines);
                let inactive = view.config.typography.inactive_opacity as f32;
                for index in range {
                    let Some(line) = lyrics.lines.get(index) else {
                        continue;
                    };
                    let distance = (index as f32 - center).abs().min(1.0);
                    let opacity = inactive + (1.0 - inactive) * (1.0 - distance);
                    if opacity <= 0.005 {
                        continue;
                    }
                    let y = metrics.lyric_y
                        + before as f64 * metrics.line_height
                        + (index as f64 - center as f64) * metrics.line_height;
                    let text = apply_case(&line.text, view.config.typography.case);
                    let mask = self.line_mask(view, metrics, index, &text);
                    let x = aligned_x(
                        view.config.typography.align,
                        metrics.lyric_x,
                        metrics.lyric_w,
                        &mask,
                    );
                    blit_mask(
                        &mut self.text,
                        metrics.window_w,
                        metrics.window_h,
                        &mask,
                        x,
                        y as i32,
                        opacity,
                        view.config.typography.shadow,
                        view.config.typography.outline,
                    );
                }
                self.draw_marker(view, metrics, before);
            }
            Content::Plain => {
                let Some(lyrics) = view.lyrics else {
                    return;
                };
                if view.config.lyrics.unsynced == iceberg_core::config::UnsyncedMode::Hide {
                    self.draw_state(
                        view,
                        metrics,
                        "No synced lyrics",
                        Some("LRCLIB only has plain text"),
                        1.0,
                    );
                    return;
                }
                let lines: Vec<&str> = lyrics
                    .plain
                    .lines()
                    .map(str::trim)
                    .filter(|line| !line.is_empty())
                    .take(metrics.visible_lines)
                    .collect();
                let label = self.state_mask(
                    view,
                    metrics,
                    "NOT SYNCED",
                    Rgba::from_hex("#f1ede3", 0.38),
                    9.0 * metrics.scale as f32,
                    600,
                    0.12,
                    metrics.lyric_w as f32,
                );
                blit_mask(
                    &mut self.text,
                    metrics.window_w,
                    metrics.window_h,
                    &label,
                    (metrics.lyric_x + metrics.lyric_w) as i32 - label.width as i32 - 4,
                    metrics.lyric_y as i32,
                    1.0,
                    false,
                    false,
                );
                for (index, line) in lines.iter().enumerate() {
                    let mask = self.plain_mask(view, metrics, index, line);
                    let y = metrics.lyric_y + (index as f64 + 1.0) * metrics.line_height;
                    let x = aligned_x(
                        view.config.typography.align,
                        metrics.lyric_x,
                        metrics.lyric_w,
                        &mask,
                    );
                    blit_mask(
                        &mut self.text,
                        metrics.window_w,
                        metrics.window_h,
                        &mask,
                        x,
                        y as i32,
                        view.config.typography.inactive_opacity as f32,
                        view.config.typography.shadow,
                        false,
                    );
                }
            }
            Content::Instrumental => {
                self.draw_state(
                    view,
                    metrics,
                    "Instrumental",
                    Some("No lyrics for this track"),
                    1.0,
                );
            }
            Content::Loading => {
                self.draw_state(view, metrics, "Looking up lyrics", Some("LRCLIB"), 1.0);
            }
            Content::NoLyrics => {
                self.draw_state(
                    view,
                    metrics,
                    "No lyrics found",
                    Some("LRCLIB has no match"),
                    1.0,
                );
            }
            Content::Error(message) => {
                self.draw_state(
                    view,
                    metrics,
                    message,
                    Some("Check the connection or try again"),
                    1.0,
                );
            }
            Content::Empty => {
                self.draw_state(
                    view,
                    metrics,
                    "Nothing playing",
                    Some("Start a song in any player"),
                    1.0,
                );
            }
        }
    }

    fn draw_state(
        &mut self,
        view: &View<'_>,
        metrics: &Metrics,
        message: &str,
        sub: Option<&str>,
        opacity: f32,
    ) {
        let family = view.font_family();
        let active = view.active_color();
        let mask = self.rasterize_text(
            message,
            &family,
            metrics.font_size as f32,
            600,
            view.config.typography.italic,
            view.config.typography.letter_spacing as f32,
            active,
            metrics.lyric_w as f32,
            1,
        );
        let y = metrics.lyric_y + metrics.line_height * 0.55;
        let x = aligned_x(
            view.config.typography.align,
            metrics.lyric_x,
            metrics.lyric_w,
            &mask,
        );
        blit_mask(
            &mut self.text,
            metrics.window_w,
            metrics.window_h,
            &mask,
            x,
            y as i32,
            opacity,
            view.config.typography.shadow,
            false,
        );
        if let Some(sub) = sub {
            let sub_mask = self.rasterize_text(
                sub,
                &family,
                metrics.font_size as f32 * 0.7,
                400,
                false,
                0.0,
                active.with_alpha(0.5),
                metrics.lyric_w as f32,
                1,
            );
            let sub_x = aligned_x(
                view.config.typography.align,
                metrics.lyric_x,
                metrics.lyric_w,
                &sub_mask,
            );
            blit_mask(
                &mut self.text,
                metrics.window_w,
                metrics.window_h,
                &sub_mask,
                sub_x,
                (y + metrics.line_height * 1.1) as i32,
                opacity,
                false,
                false,
            );
        }
    }

    fn draw_marker(&mut self, view: &View<'_>, metrics: &Metrics, before: f32) {
        if view.mode == WidgetMode::Lyrics {
            return;
        }
        let accent = view.accent();
        let width = (2.0 * metrics.scale).max(1.5);
        let height = metrics.line_height * 0.46;
        let y = metrics.lyric_y
            + before as f64 * metrics.line_height
            + (metrics.line_height - height) / 2.0;
        let x = metrics.lyric_x - 7.0 * metrics.scale;
        fill_rounded_rect(
            &mut self.text,
            metrics.window_w,
            metrics.window_h,
            x,
            y,
            width,
            height,
            width / 2.0,
            accent,
        );
    }

    fn draw_progress(&mut self, view: &View<'_>, metrics: &Metrics) {
        if !view.config.behavior.show_progress
            || view.mode != WidgetMode::Card
            || view.config.background.kind == BackgroundKind::Transparent
        {
            return;
        }
        let duration = view.playback.duration_ms;
        if duration <= 0 {
            return;
        }
        let position = view
            .playback
            .live_position_ms(view.now_ms)
            .clamp(0, duration);
        let fraction = position as f64 / duration as f64;
        if fraction <= 0.001 {
            return;
        }
        let accent = view.accent();
        fill_rounded_rect(
            &mut self.frame,
            metrics.window_w,
            metrics.window_h,
            metrics.progress_x,
            metrics.progress_y,
            (metrics.progress_w * fraction).max(metrics.progress_h),
            metrics.progress_h,
            metrics.progress_h / 2.0,
            accent,
        );
    }

    fn blend_text(&mut self, metrics: &Metrics) {
        let x0 = (metrics.lyric_x - 14.0 * metrics.scale).floor().max(0.0) as i32;
        let y0 = (metrics.lyric_y - 10.0 * metrics.scale).floor().max(0.0) as i32;
        let x1 = (metrics.lyric_x + metrics.lyric_w + 8.0 * metrics.scale).ceil() as i32;
        let y1 = (metrics.lyric_y
            + metrics.visible_lines as f64 * metrics.line_height
            + 12.0 * metrics.scale)
            .ceil() as i32;
        blend_region(
            &mut self.frame,
            &self.text,
            metrics.window_w,
            metrics.window_h,
            x0.max(0),
            y0.max(0),
            x1.min(metrics.window_w),
            y1.min(metrics.window_h),
        );
    }

    fn line_mask(
        &mut self,
        view: &View<'_>,
        metrics: &Metrics,
        index: usize,
        text: &str,
    ) -> TextMask {
        if let Some(mask) = self.line_masks.get(&index) {
            return mask.clone();
        }
        let family = view.font_family();
        let mask = self.rasterize_text(
            text,
            &family,
            metrics.font_size as f32,
            view.config.typography.weight as u16,
            view.config.typography.italic,
            view.config.typography.letter_spacing as f32,
            view.active_color(),
            metrics.lyric_w as f32,
            1,
        );
        self.line_masks.insert(index, mask.clone());
        while self.line_masks.len() > 64 {
            let key = *self.line_masks.keys().next().unwrap();
            self.line_masks.remove(&key);
        }
        mask
    }

    fn plain_mask(
        &mut self,
        view: &View<'_>,
        metrics: &Metrics,
        index: usize,
        text: &str,
    ) -> TextMask {
        if let Some(mask) = self.plain_masks.get(&index) {
            return mask.clone();
        }
        let family = view.font_family();
        let mask = self.rasterize_text(
            text,
            &family,
            metrics.font_size as f32 * 0.9,
            400,
            view.config.typography.italic,
            0.0,
            view.active_color(),
            metrics.lyric_w as f32,
            1,
        );
        self.plain_masks.insert(index, mask.clone());
        mask
    }

    #[allow(clippy::too_many_arguments)]
    fn state_mask(
        &mut self,
        view: &View<'_>,
        _metrics: &Metrics,
        text: &str,
        color: Rgba,
        size: f32,
        weight: u16,
        letter_spacing: f32,
        width: f32,
    ) -> TextMask {
        let key = format!("{text}|{size}|{weight}|{letter_spacing}|{}", color.a);
        if let Some(mask) = self.state_masks.get(&key) {
            return mask.clone();
        }
        let family = view.font_family();
        let mask = self.rasterize_text(
            text,
            &family,
            size,
            weight,
            false,
            letter_spacing,
            color,
            width,
            1,
        );
        self.state_masks.insert(key, mask.clone());
        mask
    }

    #[allow(clippy::too_many_arguments)]
    fn rasterize_text(
        &mut self,
        text: &str,
        family: &str,
        size: f32,
        weight: u16,
        italic: bool,
        letter_spacing: f32,
        color: Rgba,
        max_width: f32,
        lines: usize,
    ) -> TextMask {
        if text.trim().is_empty() {
            return TextMask {
                width: 1,
                height: 1,
                offset_x: 0,
                offset_y: 0,
                pixels: vec![0; 4],
            };
        }
        let metrics = TextMetrics::new(size.max(4.0), (size * 1.4).max(6.0));
        let pad = (size * 0.5).ceil() as i32 + 2;
        let width = max_width.ceil().max(1.0) as usize + pad as usize * 2;
        let height = (metrics.line_height * lines.max(1) as f32).ceil() as usize + pad as usize * 2;
        let mut temporary = vec![0u8; width * height * 4];
        let mut buffer = Buffer::new(&mut self.font_system, metrics);
        buffer.set_size(
            Some(max_width.max(1.0)),
            Some(metrics.line_height * lines.max(1) as f32),
        );
        buffer.set_wrap(Wrap::None);
        let attrs = Attrs::new()
            .family(Family::Name(family))
            .weight(Weight(weight))
            .style(if italic { Style::Italic } else { Style::Normal })
            .letter_spacing(letter_spacing)
            .metrics(metrics);
        buffer.set_text(text, &attrs, Shaping::Advanced, None);
        buffer.shape_until_scroll(&mut self.font_system, false);
        buffer.draw(
            &mut self.font_system,
            &mut self.swash_cache,
            Color::rgba(color.r, color.g, color.b, color.a),
            |x, y, w, h, color| {
                composite_rgba(
                    &mut temporary,
                    width as i32,
                    height as i32,
                    x + pad,
                    y + pad,
                    w,
                    h,
                    color,
                );
            },
        );
        let mut min_x = i32::MAX;
        let mut min_y = i32::MAX;
        let mut max_x = i32::MIN;
        let mut max_y = i32::MIN;
        for y in 0..height {
            for x in 0..width {
                let index = (y * width + x) * 4;
                if temporary[index + 3] > 0 {
                    min_x = min_x.min(x as i32);
                    min_y = min_y.min(y as i32);
                    max_x = max_x.max(x as i32);
                    max_y = max_y.max(y as i32);
                }
            }
        }
        if min_x == i32::MAX {
            return TextMask {
                width: 1,
                height: 1,
                offset_x: 0,
                offset_y: 0,
                pixels: vec![0; 4],
            };
        }
        let crop_w = (max_x - min_x + 1) as usize;
        let crop_h = (max_y - min_y + 1) as usize;
        let mut pixels = vec![0u8; crop_w * crop_h * 4];
        for y in 0..crop_h {
            let source_y = min_y as usize + y;
            let source_start = (source_y * width + min_x as usize) * 4;
            let destination_start = y * crop_w * 4;
            pixels[destination_start..destination_start + crop_w * 4]
                .copy_from_slice(&temporary[source_start..source_start + crop_w * 4]);
        }
        TextMask {
            width: crop_w,
            height: crop_h,
            offset_x: min_x - pad,
            offset_y: min_y - pad,
            pixels,
        }
    }
}

fn apply_case(text: &str, case: TextCase) -> String {
    match case {
        TextCase::Original => text.to_string(),
        TextCase::Upper => text.to_uppercase(),
        TextCase::Lower => text.to_lowercase(),
    }
}

pub fn visible_range(count: usize, center: f32, lines: usize) -> std::ops::RangeInclusive<usize> {
    if count == 0 || lines == 0 {
        return 0..=0;
    }
    let before = (lines / 2) as i32;
    let center_index = center.floor() as i32;
    let mut start = center_index - before;
    let mut end = start + lines as i32 - 1;
    if start < 0 {
        end -= start;
        start = 0;
    }
    if end >= count as i32 {
        let shift = end - (count as i32 - 1);
        start -= shift;
        end = count as i32 - 1;
    }
    start = start.max(0);
    end = end.min(count as i32 - 1).max(start);
    start as usize..=end as usize
}

fn background_fill(config: &Config) -> Rgba {
    let background = &config.background;
    match background.kind {
        BackgroundKind::Solid => Rgba::from_hex(&background.solid_color, background.solid_opacity),
        BackgroundKind::Transparent => {
            Rgba::from_hex(&background.solid_color, background.transparent_opacity)
        }
        // Phase A keeps the accepted CPU backdrop pipeline as a flat tint placeholder. Phase C
        // replaces these with the wallpaper blur / frost / refraction pipeline.
        BackgroundKind::Acrylic => Rgba::from_hex(
            &background.acrylic_tint,
            0.55 + background.acrylic_tint_strength * 0.35,
        ),
        BackgroundKind::Ice => {
            Rgba::from_hex(&background.ice_tint, 0.30 + background.ice_frost * 0.5)
        }
        BackgroundKind::Liquid => Rgba::from_hex(
            &background.solid_color,
            0.58 + background.liquid_highlight * 0.2,
        ),
    }
}

fn draw_shadow(target: &mut [u8], metrics: &Metrics, config: &Config) {
    let sigma = (7.0 * metrics.scale).max(1.0);
    let opacity = config.background.shadow_opacity;
    let card_x = metrics.card_x;
    let card_y = metrics.card_y;
    let half_x = metrics.card_w / 2.0;
    let half_y = metrics.card_h / 2.0;
    let center_x = card_x + half_x;
    let center_y = card_y + half_y;
    let radius = metrics.radius;
    for y in 0..metrics.window_h {
        for x in 0..metrics.window_w {
            let px = x as f64 + 0.5;
            let py = y as f64 + 0.5;
            let dx = (px - center_x).abs() - half_x + radius;
            let dy = (py - center_y).abs() - half_y + radius;
            let distance =
                (dx.max(0.0).powi(2) + dy.max(0.0).powi(2)).sqrt() + dx.max(dy).min(0.0) - radius;
            if distance < -1.0 {
                continue;
            }
            let alpha = opacity * (-(distance * distance) / (2.0 * sigma * sigma)).exp();
            if alpha <= 0.002 {
                continue;
            }
            let index = ((y * metrics.window_w + x) * 4) as usize;
            let source = (alpha.clamp(0.0, 1.0) * 255.0) as u32;
            let inverse = 255 - source;
            target[index] = (target[index] as u32 * inverse / 255) as u8;
            target[index + 1] = (target[index + 1] as u32 * inverse / 255) as u8;
            target[index + 2] = (target[index + 2] as u32 * inverse / 255) as u8;
            target[index + 3] = (source + target[index + 3] as u32 * inverse / 255).min(255) as u8;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn fill_rounded_rect(
    target: &mut [u8],
    target_w: i32,
    target_h: i32,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    radius: f64,
    color: Rgba,
) {
    if width <= 0.0 || height <= 0.0 || color.a == 0 {
        return;
    }
    let left = x.floor() as i32;
    let top = y.floor() as i32;
    let right = (x + width).ceil() as i32;
    let bottom = (y + height).ceil() as i32;
    let source_alpha = color.a as u32;
    let source_b = color.b as u32 * source_alpha / 255;
    let source_g = color.g as u32 * source_alpha / 255;
    let source_r = color.r as u32 * source_alpha / 255;
    for py in top..bottom {
        if py < 0 || py >= target_h {
            continue;
        }
        for px in left..right {
            if px < 0 || px >= target_w {
                continue;
            }
            let center_x = x + width / 2.0;
            let center_y = y + height / 2.0;
            let half_x = width / 2.0 - radius.max(0.0);
            let half_y = height / 2.0 - radius.max(0.0);
            let dx = (px as f64 + 0.5 - center_x).abs() - half_x;
            let dy = (py as f64 + 0.5 - center_y).abs() - half_y;
            let distance = (dx.max(0.0).powi(2) + dy.max(0.0).powi(2)).sqrt() + dx.max(dy).min(0.0)
                - radius.max(0.0);
            let coverage = (1.0 - distance).clamp(0.0, 1.0);
            if coverage <= 0.0 {
                continue;
            }
            let alpha = (source_alpha as f64 * coverage) as u32;
            let inverse = 255 - alpha;
            let index = ((py * target_w + px) * 4) as usize;
            target[index] =
                (source_b * alpha / 255 + target[index] as u32 * inverse / 255).min(255) as u8;
            target[index + 1] =
                (source_g * alpha / 255 + target[index + 1] as u32 * inverse / 255).min(255) as u8;
            target[index + 2] =
                (source_r * alpha / 255 + target[index + 2] as u32 * inverse / 255).min(255) as u8;
            target[index + 3] = (alpha + target[index + 3] as u32 * inverse / 255).min(255) as u8;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn stroke_rounded_rect(
    target: &mut [u8],
    target_w: i32,
    target_h: i32,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    radius: f64,
    border: f64,
    color: Rgba,
) {
    if border <= 0.0 || color.a == 0 {
        return;
    }
    let left = (x - border).floor() as i32;
    let top = (y - border).floor() as i32;
    let right = (x + width + border).ceil() as i32;
    let bottom = (y + height + border).ceil() as i32;
    let source_alpha = color.a as u32;
    let center_x = x + width / 2.0;
    let center_y = y + height / 2.0;
    let half_x = width / 2.0 - radius;
    let half_y = height / 2.0 - radius;
    for py in top..bottom {
        if py < 0 || py >= target_h {
            continue;
        }
        for px in left..right {
            if px < 0 || px >= target_w {
                continue;
            }
            let dx = (px as f64 + 0.5 - center_x).abs() - half_x;
            let dy = (py as f64 + 0.5 - center_y).abs() - half_y;
            let distance =
                (dx.max(0.0).powi(2) + dy.max(0.0).powi(2)).sqrt() + dx.max(dy).min(0.0) - radius;
            let coverage = ((border / 2.0 + 1.0) - distance.abs()).clamp(0.0, 1.0);
            if coverage <= 0.0 {
                continue;
            }
            let alpha = (source_alpha as f64 * coverage) as u32;
            let inverse = 255 - alpha;
            let index = ((py * target_w + px) * 4) as usize;
            target[index] = (color.b as u32 * alpha / 255 + target[index] as u32 * inverse / 255)
                .min(255) as u8;
            target[index + 1] = (color.g as u32 * alpha / 255
                + target[index + 1] as u32 * inverse / 255)
                .min(255) as u8;
            target[index + 2] = (color.r as u32 * alpha / 255
                + target[index + 2] as u32 * inverse / 255)
                .min(255) as u8;
            target[index + 3] = (alpha + target[index + 3] as u32 * inverse / 255).min(255) as u8;
        }
    }
}

fn aligned_x(align: Align, region_x: f64, region_w: f64, mask: &TextMask) -> i32 {
    match align {
        Align::Left => region_x as i32,
        Align::Center => {
            (region_x + (region_w - mask.width as f64) / 2.0 - mask.offset_x as f64).round() as i32
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn composite_rgba(
    target: &mut [u8],
    width: i32,
    height: i32,
    x: i32,
    y: i32,
    w: u32,
    h: u32,
    color: Color,
) {
    let source_alpha = color.a() as u32;
    if source_alpha == 0 {
        return;
    }
    let source_r = color.r() as u32 * source_alpha / 255;
    let source_g = color.g() as u32 * source_alpha / 255;
    let source_b = color.b() as u32 * source_alpha / 255;
    let inverse = 255 - source_alpha;
    for gy in 0..h as i32 {
        for gx in 0..w as i32 {
            let px = x + gx;
            let py = y + gy;
            if px < 0 || py < 0 || px >= width || py >= height {
                continue;
            }
            let index = ((py * width + px) * 4) as usize;
            target[index] = (source_b + target[index] as u32 * inverse / 255).min(255) as u8;
            target[index + 1] =
                (source_g + target[index + 1] as u32 * inverse / 255).min(255) as u8;
            target[index + 2] =
                (source_r + target[index + 2] as u32 * inverse / 255).min(255) as u8;
            target[index + 3] =
                (source_alpha + target[index + 3] as u32 * inverse / 255).min(255) as u8;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn blit_mask(
    target: &mut [u8],
    target_w: i32,
    target_h: i32,
    mask: &TextMask,
    x: i32,
    y: i32,
    opacity: f32,
    shadow: bool,
    outline: bool,
) {
    let opacity = opacity.clamp(0.0, 1.0);
    if opacity <= 0.0 {
        return;
    }
    if shadow || outline {
        let strength = if outline { 0.9 } else { 0.42 };
        let offsets: &[(i32, i32)] = if outline {
            &[(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, 1)]
        } else {
            &[(1, 2)]
        };
        for (ox, oy) in offsets {
            blit_silhouette(
                target,
                target_w,
                target_h,
                mask,
                x + ox,
                y + oy,
                strength * opacity,
            );
        }
    }
    for py in 0..mask.height as i32 {
        for px in 0..mask.width as i32 {
            let tx = x + mask.offset_x + px;
            let ty = y + mask.offset_y + py;
            if tx < 0 || ty < 0 || tx >= target_w || ty >= target_h {
                continue;
            }
            let source = (py as usize * mask.width + px as usize) * 4;
            if mask.pixels[source + 3] == 0 {
                continue;
            }
            let alpha = (mask.pixels[source + 3] as f32 * opacity) as u32;
            let inverse = 255 - alpha;
            let index = ((ty * target_w + tx) * 4) as usize;
            let sb = mask.pixels[source] as u32 * alpha / 255;
            let sg = mask.pixels[source + 1] as u32 * alpha / 255;
            let sr = mask.pixels[source + 2] as u32 * alpha / 255;
            target[index] = (sb + target[index] as u32 * inverse / 255).min(255) as u8;
            target[index + 1] = (sg + target[index + 1] as u32 * inverse / 255).min(255) as u8;
            target[index + 2] = (sr + target[index + 2] as u32 * inverse / 255).min(255) as u8;
            target[index + 3] = (alpha + target[index + 3] as u32 * inverse / 255).min(255) as u8;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn blit_silhouette(
    target: &mut [u8],
    target_w: i32,
    target_h: i32,
    mask: &TextMask,
    x: i32,
    y: i32,
    strength: f32,
) {
    for py in 0..mask.height as i32 {
        for px in 0..mask.width as i32 {
            let tx = x + mask.offset_x + px;
            let ty = y + mask.offset_y + py;
            if tx < 0 || ty < 0 || tx >= target_w || ty >= target_h {
                continue;
            }
            let source = (py as usize * mask.width + px as usize) * 4;
            let value = mask.pixels[source + 3];
            if value == 0 {
                continue;
            }
            let alpha = (value as f32 * strength) as u32;
            let inverse = 255 - alpha;
            let index = ((ty * target_w + tx) * 4) as usize;
            target[index] = (target[index] as u32 * inverse / 255) as u8;
            target[index + 1] = (target[index + 1] as u32 * inverse / 255) as u8;
            target[index + 2] = (target[index + 2] as u32 * inverse / 255) as u8;
            target[index + 3] = (alpha + target[index + 3] as u32 * inverse / 255).min(255) as u8;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn blend_region(
    target: &mut [u8],
    source: &[u8],
    width: i32,
    height: i32,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
) {
    for y in y0.max(0)..y1.min(height) {
        for x in x0.max(0)..x1.min(width) {
            let index = ((y * width + x) * 4) as usize;
            let alpha = source[index + 3] as u32;
            if alpha == 0 {
                continue;
            }
            let inverse = 255 - alpha;
            target[index] =
                (source[index] as u32 + target[index] as u32 * inverse / 255).min(255) as u8;
            target[index + 1] = (source[index + 1] as u32
                + target[index + 1] as u32 * inverse / 255)
                .min(255) as u8;
            target[index + 2] = (source[index + 2] as u32
                + target[index + 2] as u32 * inverse / 255)
                .min(255) as u8;
            target[index + 3] = (alpha + target[index + 3] as u32 * inverse / 255).min(255) as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iceberg_core::model::PlaybackStatus;

    fn config() -> Config {
        Config::default()
    }

    #[test]
    fn metrics_scale_with_dpi_and_user_scale() {
        let base = compute_metrics(&config(), WidgetMode::Card, Scale::new(1.0, 100));
        let hd = compute_metrics(&config(), WidgetMode::Card, Scale::new(1.5, 100));
        let double = compute_metrics(&config(), WidgetMode::Card, Scale::new(2.0, 100));
        assert_eq!(base.window_w, 388);
        assert_eq!(hd.window_w, 582);
        assert_eq!(double.window_w, 776);
        assert!(hd.font_size > base.font_size);
        assert!(double.line_height > hd.line_height);
    }

    #[test]
    fn user_scale_compounds_with_monitor_scale() {
        let metrics = compute_metrics(&config(), WidgetMode::Card, Scale::new(1.5, 150));
        assert!((metrics.scale - 2.25).abs() < 0.001);
        assert!(metrics.window_w > 800);
    }

    #[test]
    fn modes_have_distinct_layouts() {
        let card = compute_metrics(&config(), WidgetMode::Card, Scale::new(1.0, 100));
        let lyrics = compute_metrics(&config(), WidgetMode::Lyrics, Scale::new(1.0, 100));
        let island = compute_metrics(&config(), WidgetMode::Island, Scale::new(1.0, 100));
        assert!(card.window_h > lyrics.window_h);
        assert!(lyrics.lyric_w > 300.0);
        assert!(island.art_size > 0.0);
        assert!(island.visible_lines == 1);
    }

    #[test]
    fn visible_range_is_contiguous_and_never_repeats_an_index() {
        for count in [1usize, 2, 5, 40] {
            for center in [0.0f32, 0.5, 2.0, 20.0] {
                let range = visible_range(count, center, 5);
                let values: Vec<usize> = range.collect();
                let mut unique = values.clone();
                unique.sort();
                unique.dedup();
                assert_eq!(values.len(), unique.len());
                assert!(values.iter().all(|index| *index < count));
            }
        }
    }

    #[test]
    fn transition_eases_to_the_target() {
        let transition = Transition {
            from: 1.0,
            to: 2.0,
            started_ms: 100,
            duration_ms: 200,
        };
        assert_eq!(transition.position(100), 1.0);
        assert_eq!(transition.position(300), 2.0);
        let middle = transition.position(200);
        assert!(middle > 1.0 && middle < 2.0);
        assert!(transition.finished(300));
    }

    #[test]
    fn hex_colors_and_alpha_are_safe() {
        assert_eq!(
            Rgba::from_hex("#abc", 1.0),
            Rgba::new(0xaa, 0xbb, 0xcc, 255)
        );
        assert_eq!(Rgba::from_hex("nope", 0.5).a, 128);
        assert_eq!(Rgba::new(1, 2, 3, 4).with_alpha(0.0).a, 0);
    }

    #[test]
    fn case_transform_is_real() {
        assert_eq!(apply_case("Hello", TextCase::Upper), "HELLO");
        assert_eq!(apply_case("Hello", TextCase::Lower), "hello");
        assert_eq!(apply_case("Hello", TextCase::Original), "Hello");
    }

    #[test]
    fn renderer_composes_without_a_window() {
        let config = config();
        let track = Track {
            key: "k".to_string(),
            title: "Midnight City".to_string(),
            artist: "M83".to_string(),
            album: String::new(),
            duration_ms: 244_000,
            cover_png: None,
            accent: Some("#f07a4f".to_string()),
            source_app: "test".to_string(),
        };
        let playback = Playback {
            status: PlaybackStatus::Playing,
            position_ms: 74_000,
            duration_ms: 244_000,
            received_at_ms: 0,
            source_app: "test".to_string(),
        };
        let lyrics = Lyrics {
            track_key: track.key.clone(),
            kind: iceberg_core::LyricsKind::Synced,
            lines: vec![
                iceberg_core::LyricLine {
                    t_ms: 0,
                    text: "First line".to_string(),
                    words: Vec::new(),
                },
                iceberg_core::LyricLine {
                    t_ms: 3_000,
                    text: "Second line".to_string(),
                    words: Vec::new(),
                },
            ],
            plain: String::new(),
            source: "test".to_string(),
            message: String::new(),
        };
        let content = Content::Synced;
        let view = View {
            config: &config,
            mode: WidgetMode::Card,
            track: Some(&track),
            playback: &playback,
            lyrics: Some(&lyrics),
            content: &content,
            art: None,
            current_index: Some(1),
            transition: None,
            now_ms: 5_000,
            scale: Scale::new(1.0, 100),
        };
        let mut renderer = Renderer::new();
        assert!(renderer.ensure_size(&view));
        renderer.rebuild_background(&view);
        renderer.render(&view);
        assert_eq!(renderer.background_rebuilds, 1);
        assert!(!renderer.frame.is_empty());
        assert!(renderer.frame.chunks_exact(4).any(|pixel| pixel[3] > 0));
    }

    #[test]
    fn transition_does_not_rebuild_the_background() {
        let config = config();
        let track = Track {
            key: "k".to_string(),
            title: "Title".to_string(),
            artist: "Artist".to_string(),
            album: String::new(),
            duration_ms: 100_000,
            cover_png: None,
            accent: None,
            source_app: "test".to_string(),
        };
        let playback = Playback {
            status: PlaybackStatus::Playing,
            position_ms: 1_000,
            duration_ms: 100_000,
            received_at_ms: 0,
            source_app: "test".to_string(),
        };
        let lyrics = Lyrics {
            track_key: track.key.clone(),
            kind: iceberg_core::LyricsKind::Synced,
            lines: (0..8)
                .map(|index| iceberg_core::LyricLine {
                    t_ms: index * 1_000,
                    text: format!("Line {index}"),
                    words: Vec::new(),
                })
                .collect(),
            plain: String::new(),
            source: "test".to_string(),
            message: String::new(),
        };
        let content = Content::Synced;
        let mut renderer = Renderer::new();
        let view = View {
            config: &config,
            mode: WidgetMode::Card,
            track: Some(&track),
            playback: &playback,
            lyrics: Some(&lyrics),
            content: &content,
            art: None,
            current_index: Some(1),
            transition: Some(Transition {
                from: 1.0,
                to: 2.0,
                started_ms: 0,
                duration_ms: 200,
            }),
            now_ms: 100,
            scale: Scale::new(1.0, 100),
        };
        renderer.ensure_size(&view);
        renderer.rebuild_background(&view);
        for step in 0..10 {
            let frame = View {
                now_ms: 100 + step * 20,
                ..view
            };
            renderer.render(&frame);
        }
        assert_eq!(renderer.background_rebuilds, 1);
    }
}
