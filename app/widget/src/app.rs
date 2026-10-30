use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use iceberg_core::config::{Anchor, Config, TransitionStyle, WidgetMode};
use iceberg_core::fonts::{stock_font_dir, FontEntry, FontLibrary};
use iceberg_core::lyrics::LyricsService;
use iceberg_core::media::{MediaEvent, MediaWatcher};
use iceberg_core::model::{Lyrics, LyricsKind, Playback, PlaybackStatus, Track};
use iceberg_core::paths::Paths;
use iceberg_core::util::now_ms;
use serde::Serialize;
use windows::Win32::Foundation::HWND;
use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::platform::windows::WindowAttributesExtWindows;
use winit::window::{Window, WindowLevel};

use crate::demo;
use crate::dpi::{self, Scale};
use crate::perf::{self, CpuTimes, MemoryStats};
use crate::render::{compute_metrics, ArtImage, Content, Renderer, Transition, View};
use crate::tray::{Tray, TrayAction};
use crate::window::{self, LayeredSurface, StyleReport};

#[derive(Clone, Debug)]
pub enum AppEvent {
    Media(MediaEvent),
    Lyrics { key: String, lyrics: Lyrics },
    Tray(TrayAction),
}

#[derive(Clone, Debug, Default)]
pub struct Options {
    pub demo: bool,
    pub bench: Option<PathBuf>,
    pub screenshot: Option<PathBuf>,
    pub scale_override: Option<f64>,
    pub click_through: Option<bool>,
    pub quit_after: Option<Duration>,
    pub config_path: Option<PathBuf>,
    pub mode: Option<WidgetMode>,
}

pub fn run(options: Options) -> Result<(), String> {
    let paths = Paths::resolve();
    paths.ensure().map_err(|error| error.to_string())?;
    let config_path = options
        .config_path
        .clone()
        .unwrap_or_else(|| paths.config.clone());
    let mut config = Config::load(&config_path);
    config.validate();
    if let Some(mode) = options.mode {
        config.mode = mode;
    }
    if let Some(click_through) = options.click_through {
        config.behavior.click_through = click_through;
    }
    if !config_path.exists() {
        let _ = config.save(&config_path);
    }

    if let Some(directory) = &options.screenshot {
        return crate::shot::render_screenshots(&config, directory);
    }

    let instance = if options.bench.is_none() {
        Some(
            iceberg_core::single_instance::acquire("iceberg-widget")
                .ok_or_else(|| "Iceberg is already running".to_string())?,
        )
    } else {
        None
    };

    let event_loop = EventLoop::<AppEvent>::with_user_event()
        .build()
        .map_err(|error| error.to_string())?;
    let proxy = event_loop.create_proxy();
    let mut app = App::new(options, config, paths, proxy);
    event_loop
        .run_app(&mut app)
        .map_err(|error| error.to_string())?;
    drop(instance);
    Ok(())
}

struct BenchState {
    output: PathBuf,
    started: Instant,
    stage: u8,
    next_transition: Instant,
    transitions: u32,
    transition_cpu_start: Option<CpuTimes>,
    transition_cpu_end: Option<CpuTimes>,
    idle_cpu_start: Option<CpuTimes>,
    idle_cpu_end: Option<CpuTimes>,
    background_start: u64,
    background_end: u64,
    idle_start: Instant,
    idle_end: Instant,
}

#[derive(Serialize)]
struct BenchReport {
    renderer: &'static str,
    os_scale: f64,
    effective_scale: f64,
    surface: [i32; 2],
    dpi_awareness: &'static str,
    style: Option<StyleReport>,
    transitions: u32,
    transition_seconds: f64,
    transition_cpu_one_core: f64,
    transition_cpu_total: f64,
    idle_seconds: f64,
    idle_cpu_one_core: f64,
    idle_cpu_total: f64,
    background_rebuilds_before: u64,
    background_rebuilds_during_transitions: u64,
    background_rebuilds_total: u64,
    memory: MemoryStats,
    note: &'static str,
}

pub struct App {
    options: Options,
    config: Config,
    paths: Paths,
    proxy: EventLoopProxy<AppEvent>,
    config_shared: Arc<Mutex<Config>>,
    window: Option<Window>,
    surface: Option<LayeredSurface>,
    hwnd: Option<HWND>,
    renderer: Option<Renderer>,
    font_library: FontLibrary,
    font_entry: Option<Arc<FontEntry>>,
    font_family: String,
    font_request: String,
    scale: Scale,
    monitor_name: String,
    style: Option<StyleReport>,
    track: Option<Track>,
    playback: Playback,
    lyrics: Option<Lyrics>,
    content: Content,
    art: Option<ArtImage>,
    current_index: Option<usize>,
    transition: Option<Transition>,
    transition_end: Option<Instant>,
    background_dirty: bool,
    text_dirty: bool,
    media: Option<MediaWatcher>,
    tray: Option<Tray>,
    user_hidden: bool,
    next_line: Option<Instant>,
    next_progress: Option<Instant>,
    next_frame: Option<Instant>,
    quit_at: Option<Instant>,
    trim_at: Option<Instant>,
    fetching_key: Option<String>,
    bench: Option<BenchState>,
}

impl App {
    fn new(
        options: Options,
        config: Config,
        paths: Paths,
        proxy: EventLoopProxy<AppEvent>,
    ) -> Self {
        let shared = Arc::new(Mutex::new(config.clone()));
        let quit_at = options.quit_after.map(|delay| Instant::now() + delay);
        let font_library = FontLibrary::new(stock_font_dir(), paths.fonts.clone());
        Self {
            options,
            config,
            paths,
            proxy,
            config_shared: shared,
            window: None,
            surface: None,
            hwnd: None,
            renderer: None,
            font_library,
            font_entry: None,
            font_family: "Lato".to_string(),
            font_request: String::new(),
            scale: Scale::new(1.0, 100),
            monitor_name: String::new(),
            style: None,
            track: None,
            playback: Playback::default(),
            lyrics: None,
            content: Content::Empty,
            art: None,
            current_index: None,
            transition: None,
            transition_end: None,
            background_dirty: true,
            text_dirty: true,
            media: None,
            tray: None,
            user_hidden: false,
            next_line: None,
            next_progress: None,
            next_frame: None,
            quit_at,
            trim_at: None,
            fetching_key: None,
            bench: None,
        }
    }

    fn effective(&self) -> Config {
        self.config.effective()
    }

    fn resolve_active_font(&mut self, effective: &Config) -> Option<Arc<FontEntry>> {
        let requested = self
            .track
            .as_ref()
            .map(|track| effective.font_for(&track.key))
            .unwrap_or_else(|| effective.typography.font_family.clone());
        if requested != self.font_request {
            self.font_request = requested;
            let entry = self
                .font_library
                .resolve(&self.font_request)
                .or_else(|| self.font_library.resolve("Lato"));
            self.font_family = entry
                .as_ref()
                .map(|entry| entry.info.family.clone())
                .unwrap_or_else(|| "Lato".to_string());
            self.font_entry = entry;
            self.background_dirty = true;
            self.text_dirty = true;
        }
        self.font_entry.clone()
    }

    fn font_metrics(&mut self, effective: &Config) -> iceberg_core::fonts::FontMetrics {
        self.resolve_active_font(effective)
            .map(|entry| entry.metrics)
            .unwrap_or_default()
    }

    fn ensure_window(&mut self, event_loop: &ActiveEventLoop) -> Result<(), String> {
        if self.window.is_some() {
            return Ok(());
        }
        let effective = self.effective();
        let os_scale = self.options.scale_override.unwrap_or_else(|| {
            event_loop
                .primary_monitor()
                .map(|monitor| monitor.scale_factor())
                .unwrap_or(1.0)
        });
        self.scale = Scale::new(os_scale, effective.layout.scale);
        let font_metrics = self.font_metrics(&effective);
        let metrics = compute_metrics(&effective, effective.mode, self.scale, font_metrics);
        let attributes = Window::default_attributes()
            .with_title("Iceberg")
            .with_inner_size(PhysicalSize::new(
                metrics.window_w.max(1) as u32,
                metrics.window_h.max(1) as u32,
            ))
            .with_position(PhysicalPosition::new(-10_000, -10_000))
            .with_decorations(false)
            .with_transparent(true)
            .with_resizable(false)
            .with_active(false)
            .with_skip_taskbar(true)
            .with_window_level(WindowLevel::AlwaysOnTop);
        let window = event_loop
            .create_window(attributes)
            .map_err(|error| error.to_string())?;
        let hwnd = window::hwnd_of(&window);
        let style = window::apply_styles(hwnd, effective.behavior.click_through);
        let work = window::work_area(hwnd);
        let name = window
            .current_monitor()
            .and_then(|monitor| monitor.name())
            .unwrap_or_default();
        let (anchor, offset_x, offset_y) = monitor_layout(&effective, &name);
        let scale = self.scale.effective();
        let (x, y) = window::anchored_position(
            work,
            metrics.window_w,
            metrics.window_h,
            metrics.card_w as i32,
            metrics.card_h as i32,
            metrics.margin as i32,
            anchor,
            (offset_x as f64 * scale).round() as i32,
            (offset_y as f64 * scale).round() as i32,
        );
        window::place(hwnd, x, y, metrics.window_w.max(1), metrics.window_h.max(1));
        window::show_no_activate(hwnd);
        let surface = LayeredSurface::new(metrics.window_w, metrics.window_h)?;
        self.surface = Some(surface);
        if self.renderer.is_none() {
            self.renderer = Some(Renderer::new());
        }
        self.hwnd = Some(hwnd);
        self.window = Some(window);
        self.monitor_name = name;
        self.style = Some(style);
        self.trim_at = None;
        self.background_dirty = true;
        self.text_dirty = true;
        self.refresh_art();
        eprintln!("iceberg: window {:?}", style);
        if let Some(window) = &self.window {
            window.request_redraw();
        }
        Ok(())
    }

    fn destroy_window(&mut self) {
        self.window = None;
        self.surface = None;
        self.hwnd = None;
        self.art = None;
        self.transition = None;
        self.transition_end = None;
        self.next_frame = None;
        self.next_line = None;
        self.next_progress = None;
        self.renderer = None;
        self.font_entry = None;
        self.font_library.clear();
        self.font_request.clear();
        self.font_family = "Lato".to_string();
        self.trim_at = Some(Instant::now() + Duration::from_millis(300));
    }

    fn refresh_art(&mut self) {
        let Some(track) = &self.track else {
            self.art = None;
            return;
        };
        let Some(bytes) = track.cover_png.as_deref() else {
            self.art = None;
            return;
        };
        let target = self
            .renderer
            .as_ref()
            .and_then(|renderer| renderer.metrics.as_ref())
            .map(|metrics| metrics.art_size.round() as usize)
            .unwrap_or(44);
        self.art = ArtImage::from_png(bytes, target);
        self.background_dirty = true;
    }

    fn seed_demo(&mut self) {
        self.track = Some(demo::track());
        self.playback = demo::playback();
        self.lyrics = Some(demo::lyrics());
        self.content = Content::Synced;
        self.current_index = Some(2);
        self.transition = None;
        self.background_dirty = true;
        self.text_dirty = true;
        let effective = self.effective();
        self.resolve_active_font(&effective);
        self.update_timers();
    }

    fn on_track(&mut self, track: Track, playback: Playback) {
        self.track = Some(track.clone());
        self.playback = playback;
        self.lyrics = None;
        self.content = Content::Loading;
        self.current_index = None;
        self.transition = None;
        self.transition_end = None;
        self.art = None;
        self.refresh_art();
        self.background_dirty = true;
        self.text_dirty = true;
        self.fetching_key = None;
        let effective = self.effective();
        self.resolve_active_font(&effective);
        self.start_lyrics_fetch(&track);
        self.recompute_index(false);
        self.update_timers();
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn start_lyrics_fetch(&mut self, track: &Track) {
        if self.fetching_key.as_deref() == Some(track.key.as_str()) {
            return;
        }
        self.fetching_key = Some(track.key.clone());
        let cache = self.paths.cache.clone();
        let proxy = self.proxy.clone();
        let track = track.clone();
        std::thread::Builder::new()
            .name("iceberg-lyrics".into())
            .spawn(move || {
                let service = LyricsService::new(cache);
                let lyrics = service.fetch_for(&track, false);
                let _ = proxy.send_event(AppEvent::Lyrics {
                    key: track.key,
                    lyrics,
                });
            })
            .ok();
    }

    fn on_lyrics(&mut self, key: String, lyrics: Lyrics) {
        let Some(track) = &self.track else {
            return;
        };
        if track.key != key {
            return;
        }
        self.fetching_key = None;
        self.content = match lyrics.kind {
            LyricsKind::Synced => Content::Synced,
            LyricsKind::Plain => Content::Plain,
            LyricsKind::Instrumental => Content::Instrumental,
            LyricsKind::None => Content::NoLyrics,
            LyricsKind::Error => Content::Error(lyrics.message.clone()),
        };
        self.lyrics = Some(lyrics);
        self.current_index = None;
        self.transition = None;
        self.transition_end = None;
        self.text_dirty = true;
        self.recompute_index(false);
        self.update_timers();
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn recompute_index(&mut self, animate: bool) {
        let effective = self.effective();
        let Some(track) = &self.track else {
            self.current_index = None;
            return;
        };
        let Some(lyrics) = &self.lyrics else {
            self.current_index = None;
            return;
        };
        if lyrics.kind != LyricsKind::Synced || lyrics.lines.is_empty() {
            self.current_index = None;
            return;
        }
        let offset = effective.effective_offset(&track.key);
        let position = (self.playback.live_position_ms(now_ms()) + offset).max(0);
        let mut index = 0usize;
        for (line_index, line) in lyrics.lines.iter().enumerate() {
            if line.t_ms <= position {
                index = line_index;
            } else {
                break;
            }
        }
        let previous = self.current_index;
        let target = Some(index);
        if previous != target {
            let can_animate = animate
                && effective.typography.transition != TransitionStyle::Off
                && previous
                    .map(|value| (value as i64 - index as i64).abs() <= 3)
                    .unwrap_or(false);
            if let Some(previous_index) = previous.filter(|_| can_animate) {
                let duration = effective.typography.transition_ms.max(1);
                self.transition = Some(Transition {
                    from: previous_index as f32,
                    to: index as f32,
                    started_ms: now_ms(),
                    duration_ms: duration,
                });
                self.transition_end = Some(Instant::now() + Duration::from_millis(duration as u64));
                self.next_frame = Some(Instant::now());
            } else {
                self.current_index = target;
                self.transition = None;
                self.transition_end = None;
            }
        }
        self.update_timers();
    }

    fn update_timers(&mut self) {
        self.schedule_line_deadline();
        self.schedule_progress();
    }

    fn schedule_line_deadline(&mut self) {
        self.next_line = None;
        let effective = self.effective();
        if self.playback.status != PlaybackStatus::Playing {
            return;
        }
        if !matches!(self.content, Content::Synced) {
            return;
        }
        let (Some(track), Some(lyrics), Some(index)) =
            (&self.track, &self.lyrics, self.current_index)
        else {
            return;
        };
        if lyrics.kind != LyricsKind::Synced {
            return;
        }
        let Some(next) = lyrics.lines.get(index + 1) else {
            return;
        };
        let offset = effective.effective_offset(&track.key);
        let position = (self.playback.live_position_ms(now_ms()) + offset).max(0);
        let wait = (next.t_ms - offset - position).max(16);
        self.next_line = Some(Instant::now() + Duration::from_millis(wait as u64));
    }

    fn schedule_progress(&mut self) {
        self.next_progress = None;
        let effective = self.effective();
        if self.playback.status != PlaybackStatus::Playing
            || effective.mode != WidgetMode::Card
            || !effective.behavior.show_progress
            || self.playback.duration_ms <= 0
        {
            return;
        }
        let font_metrics = self
            .font_entry
            .as_ref()
            .map(|entry| entry.metrics)
            .unwrap_or_default();
        let metrics = compute_metrics(&effective, effective.mode, self.scale, font_metrics);
        let bar = metrics.progress_w.max(1.0);
        let position = self
            .playback
            .live_position_ms(now_ms())
            .min(self.playback.duration_ms);
        let fraction = position as f64 / self.playback.duration_ms as f64;
        let pixel = (fraction * bar).floor();
        let next_position = (pixel + 1.0) * self.playback.duration_ms as f64 / bar;
        let wait = (next_position - position as f64).max(32.0);
        self.next_progress = Some(Instant::now() + Duration::from_millis(wait as u64));
    }

    fn draw(&mut self) {
        let effective = self.effective();
        if self.window.is_none() {
            return;
        }
        let font_entry = self.resolve_active_font(&effective);
        let font_metrics = font_entry
            .as_ref()
            .map(|entry| entry.metrics)
            .unwrap_or_default();
        let metrics = compute_metrics(&effective, effective.mode, self.scale, font_metrics);
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };
        let size_changed = renderer.metrics.as_ref() != Some(&metrics);
        if size_changed {
            if let Some(hwnd) = self.hwnd {
                window::resize(hwnd, metrics.window_w, metrics.window_h);
            }
            if let Some(surface) = &mut self.surface {
                if let Err(error) = surface.resize(metrics.window_w, metrics.window_h) {
                    eprintln!("iceberg: surface resize failed: {error}");
                }
            }
            self.art = self.track.as_ref().and_then(|track| {
                track
                    .cover_png
                    .as_deref()
                    .and_then(|bytes| ArtImage::from_png(bytes, metrics.art_size.round() as usize))
            });
            self.background_dirty = true;
            self.text_dirty = true;
        }
        let view = View {
            config: &effective,
            mode: effective.mode,
            track: self.track.as_ref(),
            playback: &self.playback,
            lyrics: self.lyrics.as_ref(),
            content: &self.content,
            art: self.art.as_ref(),
            current_index: self.current_index,
            transition: self.transition,
            now_ms: now_ms(),
            scale: self.scale,
            font_family: &self.font_family,
            font_metrics,
            font_entry: font_entry.as_deref(),
        };
        renderer.ensure_size(&view);
        if self.background_dirty {
            renderer.rebuild_background(&view);
            self.background_dirty = false;
        }
        if self.text_dirty {
            renderer.clear_text_cache();
            self.text_dirty = false;
        }
        renderer.render(&view);
        if let (Some(surface), Some(hwnd)) = (&mut self.surface, self.hwnd) {
            let frame = renderer.frame();
            if surface.pixels_mut().len() == frame.len() {
                surface.pixels_mut().copy_from_slice(frame);
                surface.present(hwnd);
            }
        }
    }

    fn handle_media(&mut self, event_loop: &ActiveEventLoop, event: MediaEvent) {
        match event {
            MediaEvent::Track { track, playback } => {
                if let Err(error) = self.ensure_window(event_loop) {
                    eprintln!("iceberg: could not create the widget window: {error}");
                    return;
                }
                if self.user_hidden {
                    self.user_hidden = false;
                }
                if let Some(window) = &self.window {
                    window::show_no_activate(window::hwnd_of(window));
                }
                self.on_track(track, playback);
            }
            MediaEvent::Playback(playback) => {
                self.playback = playback;
                self.recompute_index(true);
                self.update_timers();
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            MediaEvent::Stopped | MediaEvent::NoSession => {
                self.track = None;
                self.lyrics = None;
                self.art = None;
                self.content = Content::Empty;
                self.current_index = None;
                self.transition = None;
                self.transition_end = None;
                self.fetching_key = None;
                self.background_dirty = true;
                self.text_dirty = true;
                if self.window.is_some() && (self.options.demo || self.options.bench.is_some()) {
                    let effective = self.effective();
                    self.resolve_active_font(&effective);
                }
                self.update_timers();
                if !self.options.demo && self.options.bench.is_none() {
                    self.destroy_window();
                }
            }
            MediaEvent::Error(message) => {
                if self.track.is_some() {
                    self.content = Content::Error(message);
                    self.text_dirty = true;
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
            }
        }
    }

    fn handle_tray(&mut self, event_loop: &ActiveEventLoop, action: TrayAction) {
        match action {
            TrayAction::ToggleVisibility => {
                if self.user_hidden {
                    self.user_hidden = false;
                    if let Err(error) = self.ensure_window(event_loop) {
                        eprintln!("iceberg: could not show the widget: {error}");
                        return;
                    }
                    if let Some(hwnd) = self.hwnd {
                        window::show_no_activate(hwnd);
                    }
                    self.recompute_index(false);
                    self.update_timers();
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                } else {
                    self.user_hidden = true;
                    self.next_line = None;
                    self.next_progress = None;
                    self.next_frame = None;
                    self.transition = None;
                    self.transition_end = None;
                    if let Some(hwnd) = self.hwnd {
                        window::hide(hwnd);
                    }
                    self.trim_at = Some(Instant::now() + Duration::from_millis(300));
                }
            }
            TrayAction::Quit => event_loop.exit(),
        }
    }

    fn bench_tick(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        let mut finish = false;
        let mut exit = false;
        {
            let Some(bench) = self.bench.as_mut() else {
                return;
            };
            let elapsed = now.duration_since(bench.started).as_secs_f64();
            match bench.stage {
                0 => {
                    if elapsed >= 1.0 {
                        bench.stage = 1;
                        bench.transition_cpu_start = Some(perf::cpu_times());
                        bench.background_start = self
                            .renderer
                            .as_ref()
                            .map(|renderer| renderer.background_rebuilds)
                            .unwrap_or(0);
                        bench.next_transition = now;
                        event_loop.set_control_flow(ControlFlow::WaitUntil(now));
                    } else {
                        event_loop.set_control_flow(ControlFlow::WaitUntil(
                            bench.started + Duration::from_secs_f64(1.0),
                        ));
                    }
                }
                1 => {
                    if let Some(transition) = self.transition {
                        if transition.finished(now_ms()) {
                            self.current_index = Some(transition.to.round() as usize);
                            self.transition = None;
                            self.transition_end = None;
                            self.next_frame = None;
                        }
                    }
                    if now >= bench.next_transition && bench.transitions < 12 {
                        let line_count = self
                            .lyrics
                            .as_ref()
                            .map(|lyrics| lyrics.lines.len())
                            .unwrap_or(1)
                            .max(1);
                        let current = self.current_index.unwrap_or(0);
                        let next = (current + 1) % line_count;
                        let duration = 200u32;
                        self.transition = Some(Transition {
                            from: current as f32,
                            to: next as f32,
                            started_ms: now_ms(),
                            duration_ms: duration,
                        });
                        self.transition_end = Some(now + Duration::from_millis(duration as u64));
                        self.next_frame = Some(now);
                        bench.transitions += 1;
                        bench.next_transition = now + Duration::from_millis(240);
                        if let Some(window) = &self.window {
                            window.request_redraw();
                        }
                    }
                    if self.transition.is_some()
                        && self.next_frame.map(|frame| now >= frame).unwrap_or(true)
                    {
                        if let Some(window) = &self.window {
                            window.request_redraw();
                        }
                        self.next_frame = Some(now + Duration::from_millis(16));
                    }
                    let mut next = bench.next_transition;
                    if let Some(frame) = self.next_frame {
                        next = next.min(frame);
                    }
                    if let Some(end) = self.transition_end {
                        next = next.min(end);
                    }
                    let stage_end = bench.started + Duration::from_secs_f64(3.9);
                    next = next.min(stage_end);
                    if now >= stage_end {
                        bench.stage = 2;
                        bench.transition_cpu_end = Some(perf::cpu_times());
                        bench.background_end = self
                            .renderer
                            .as_ref()
                            .map(|renderer| renderer.background_rebuilds)
                            .unwrap_or(0);
                        bench.idle_cpu_start = Some(perf::cpu_times());
                        bench.idle_start = now;
                        bench.idle_end = now + Duration::from_secs(2);
                        self.transition = None;
                        self.transition_end = None;
                        self.next_frame = None;
                        if let Some(window) = &self.window {
                            window.request_redraw();
                        }
                    }
                    event_loop.set_control_flow(ControlFlow::WaitUntil(next));
                }
                2 => {
                    if now >= bench.idle_end {
                        bench.idle_cpu_end = Some(perf::cpu_times());
                        bench.stage = 3;
                        finish = true;
                        exit = true;
                    } else {
                        event_loop.set_control_flow(ControlFlow::WaitUntil(bench.idle_end));
                    }
                }
                _ => {
                    exit = true;
                }
            }
        }
        if finish {
            self.finish_bench();
        }
        if exit {
            event_loop.exit();
        }
    }

    fn finish_bench(&mut self) {
        let Some(bench) = self.bench.take() else {
            return;
        };
        let transition_start = bench.transition_cpu_start.unwrap_or_default();
        let transition_end = bench.transition_cpu_end.unwrap_or_default();
        let idle_start = bench.idle_cpu_start.unwrap_or_default();
        let idle_end = bench.idle_cpu_end.unwrap_or_default();
        let transition_seconds = 2.9;
        let idle_seconds = 2.0;
        let cores = std::thread::available_parallelism()
            .map(|value| value.get())
            .unwrap_or(1) as f64;
        let transition_delta = transition_end.total_ms() - transition_start.total_ms();
        let idle_delta = idle_end.total_ms() - idle_start.total_ms();
        let metrics = self
            .renderer
            .as_ref()
            .and_then(|renderer| renderer.metrics.clone());
        let report = BenchReport {
            renderer: "winit + UpdateLayeredWindow + cosmic-text/swash",
            os_scale: self.scale.os,
            effective_scale: self.scale.effective(),
            surface: metrics
                .as_ref()
                .map(|metrics| [metrics.window_w, metrics.window_h])
                .unwrap_or([0, 0]),
            dpi_awareness: dpi::awareness_label(),
            style: self.style,
            transitions: bench.transitions,
            transition_seconds,
            transition_cpu_one_core: transition_delta / transition_seconds / 10.0,
            transition_cpu_total: transition_delta / transition_seconds / 10.0 / cores,
            idle_seconds,
            idle_cpu_one_core: idle_delta / idle_seconds / 10.0,
            idle_cpu_total: idle_delta / idle_seconds / 10.0 / cores,
            background_rebuilds_before: bench.background_start,
            background_rebuilds_during_transitions: bench.background_end - bench.background_start,
            background_rebuilds_total: self
                .renderer
                .as_ref()
                .map(|renderer| renderer.background_rebuilds)
                .unwrap_or(0),
            memory: perf::memory_stats(),
            note: "Transition window includes 12 line changes at 240 ms spacing with 200 ms cached-text transitions.",
        };
        if let Some(parent) = bench.output.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_vec_pretty(&report) {
            let _ = std::fs::write(&bench.output, &json);
            #[cfg(debug_assertions)]
            {
                println!("{}", String::from_utf8_lossy(&json));
            }
        }
    }
}

impl ApplicationHandler<AppEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.options.demo || self.options.bench.is_some() {
            self.seed_demo();
            if let Err(error) = self.ensure_window(event_loop) {
                eprintln!("iceberg: could not create the widget window: {error}");
                event_loop.exit();
                return;
            }
        } else {
            let proxy = self.proxy.clone();
            let cache = self.paths.cache.clone();
            let config = self.config_shared.clone();
            self.media = Some(MediaWatcher::start(cache, config, move |event| {
                let _ = proxy.send_event(AppEvent::Media(event));
            }));
        }
        if self.options.bench.is_none() {
            match Tray::new(self.proxy.clone()) {
                Ok(tray) => self.tray = Some(tray),
                Err(error) => eprintln!("iceberg: tray unavailable: {error}"),
            }
        }
        if let Some(output) = &self.options.bench {
            let start = Instant::now();
            self.bench = Some(BenchState {
                output: output.clone(),
                started: start,
                stage: 0,
                next_transition: start,
                transitions: 0,
                transition_cpu_start: None,
                transition_cpu_end: None,
                idle_cpu_start: None,
                idle_cpu_end: None,
                background_start: 0,
                background_end: 0,
                idle_start: start,
                idle_end: start,
            });
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }
        if self.window.is_none() {
            self.trim_at = Some(Instant::now() + Duration::from_secs(1));
        }
        event_loop.set_control_flow(ControlFlow::Wait);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::RedrawRequested => self.draw(),
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                if self.options.scale_override.is_none() {
                    let effective = self.effective();
                    self.scale = Scale::new(scale_factor, effective.layout.scale);
                    self.background_dirty = true;
                    self.text_dirty = true;
                    self.refresh_art();
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
            }
            WindowEvent::Moved(_) => {
                if let Some(window) = &self.window {
                    let name = window
                        .current_monitor()
                        .and_then(|monitor| monitor.name())
                        .unwrap_or_default();
                    if name != self.monitor_name {
                        self.monitor_name = name;
                        if self.options.scale_override.is_none() {
                            let os = window.scale_factor();
                            let effective = self.effective();
                            self.scale = Scale::new(os, effective.layout.scale);
                            self.background_dirty = true;
                            self.text_dirty = true;
                            window.request_redraw();
                        }
                    }
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                if let Some(window) = &self.window {
                    let _ = window.drag_window();
                }
            }
            _ => {}
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: AppEvent) {
        match event {
            AppEvent::Media(event) => self.handle_media(event_loop, event),
            AppEvent::Lyrics { key, lyrics } => self.on_lyrics(key, lyrics),
            AppEvent::Tray(action) => self.handle_tray(event_loop, action),
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(quit_at) = self.quit_at {
            if Instant::now() >= quit_at {
                event_loop.exit();
                return;
            }
        }
        if self.bench.is_some() {
            self.bench_tick(event_loop);
            return;
        }
        if self.window.is_none() || self.user_hidden {
            if let Some(trim_at) = self.trim_at {
                if Instant::now() >= trim_at {
                    perf::trim_working_set();
                    self.trim_at = None;
                }
            }
            let deadline = [self.quit_at, self.trim_at].into_iter().flatten().min();
            match deadline {
                Some(deadline) => {
                    event_loop.set_control_flow(ControlFlow::WaitUntil(deadline));
                }
                None => event_loop.set_control_flow(ControlFlow::Wait),
            }
            return;
        }
        let now = Instant::now();
        let mut next: Option<Instant> = None;
        if let Some(transition) = self.transition {
            if transition.finished(now_ms()) {
                self.current_index = Some(transition.to.round() as usize);
                self.transition = None;
                self.transition_end = None;
                self.next_frame = None;
                self.update_timers();
            } else {
                if self.next_frame.map(|frame| now >= frame).unwrap_or(true) {
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                    self.next_frame = Some(now + Duration::from_millis(16));
                }
                next = self.next_frame;
                if let Some(end) = self.transition_end {
                    next = Some(next.map(|value| value.min(end)).unwrap_or(end));
                }
            }
        }
        if let Some(deadline) = self.next_line {
            if now >= deadline {
                self.recompute_index(true);
            }
            if let Some(deadline) = self.next_line {
                next = Some(next.map(|value| value.min(deadline)).unwrap_or(deadline));
            }
        }
        if let Some(deadline) = self.next_progress {
            if now >= deadline {
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
                self.schedule_progress();
            }
            if let Some(deadline) = self.next_progress {
                next = Some(next.map(|value| value.min(deadline)).unwrap_or(deadline));
            }
        }
        if let Some(quit_at) = self.quit_at {
            next = Some(next.map(|value| value.min(quit_at)).unwrap_or(quit_at));
        }
        match next {
            Some(deadline) => event_loop.set_control_flow(ControlFlow::WaitUntil(deadline)),
            None => event_loop.set_control_flow(ControlFlow::Wait),
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(media) = &mut self.media {
            media.stop();
        }
        self.tray = None;
    }
}

fn monitor_layout(config: &Config, name: &str) -> (Anchor, i32, i32) {
    if let Some(monitor) = config.layout.per_monitor.get(name) {
        (monitor.anchor, monitor.offset_x, monitor.offset_y)
    } else {
        (
            config.layout.anchor,
            config.layout.offset_x,
            config.layout.offset_y,
        )
    }
}
