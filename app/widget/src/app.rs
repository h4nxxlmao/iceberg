use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use iceberg_core::config::{
    Anchor, BackgroundKind, Config, MonitorLayout, TransitionStyle, WidgetMode,
};
use iceberg_core::fonts::{stock_font_dir, FontEntry, FontLibrary};
use iceberg_core::ipc::{IpcStatus, TrackInfo};
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

use crate::backdrop::{needs_wallpaper, wallpaper_path, BackdropCrop, Wallpaper};
use crate::demo;
use crate::dpi::{self, Scale};
use crate::perf::{self, CpuTimes, MemoryStats};
use crate::render::{compute_metrics, ArtImage, Content, Metrics, Renderer, Transition, View};
use crate::tray::{HotkeyAction, Tray, TrayAction};
use crate::window::{self, LayeredSurface, StyleReport};

#[derive(Clone, Debug)]
#[allow(clippy::large_enum_variant)] // Event-loop messages are transient, not a hot allocation path.
pub enum AppEvent {
    Media(MediaEvent),
    Lyrics { key: String, lyrics: Lyrics },
    Tray(TrayAction),
    Config(Config),
    SettingsOpen(bool),
    ShowWidget,
    FontsChanged,
    RefreshLyrics,
    Visibility(bool),
    Fullscreen(bool),
}

#[derive(Clone, Debug, Default)]
pub struct Options {
    pub demo: bool,
    pub bench: Option<PathBuf>,
    pub screenshot: Option<PathBuf>,
    pub settings_assets: Option<PathBuf>,
    pub font_previews: Option<PathBuf>,
    pub preview_text: Option<String>,
    pub scale_override: Option<f64>,
    pub click_through: Option<bool>,
    pub quit_after: Option<Duration>,
    pub config_path: Option<PathBuf>,
    pub mode: Option<WidgetMode>,
    pub background: Option<BackgroundKind>,
}

pub fn run(options: Options) -> Result<(), String> {
    if let Some(directory) = &options.font_previews {
        return crate::shot::render_font_previews(
            directory,
            options
                .preview_text
                .as_deref()
                .unwrap_or("The night city grows"),
        );
    }
    let paths = Paths::resolve();
    paths.ensure().map_err(|error| error.to_string())?;
    let config_path = options
        .config_path
        .clone()
        .unwrap_or_else(|| paths.config.clone());
    let mut config = Config::load(&config_path);
    config.validate();
    // Rewrite on every start so prototype-era files are migrated to the current schema.
    let _ = config.save(&config_path);
    if let Some(mode) = options.mode {
        config.mode = mode;
    }
    if let Some(click_through) = options.click_through {
        config.behavior.click_through = click_through;
    }
    if let Some(background) = options.background {
        config.background.kind = background;
    }

    if let Some(directory) = &options.screenshot {
        return crate::shot::render_screenshots(&config, directory);
    }
    if let Some(directory) = &options.settings_assets {
        return crate::shot::render_settings_assets(directory);
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

struct ConfigFade {
    old: Vec<u8>,
    width: usize,
    height: usize,
    started: Instant,
    duration: Duration,
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
    transition_started: Instant,
}

#[derive(Serialize)]
struct BenchReport {
    renderer: &'static str,
    os_scale: f64,
    effective_scale: f64,
    surface: [i32; 2],
    dpi_awareness: &'static str,
    style: Option<StyleReport>,
    background: &'static str,
    backdrop: &'static str,
    background_build_ms: f64,
    first_background_ms: f64,
    backdrop_pipeline_ms: f64,
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
    config_path: PathBuf,
    proxy: EventLoopProxy<AppEvent>,
    config_shared: Arc<Mutex<Config>>,
    status_shared: Arc<Mutex<IpcStatus>>,
    settings_open: bool,
    settings_demo: bool,
    ipc_started: bool,
    window: Option<Window>,
    surface: Option<LayeredSurface>,
    hwnd: Option<HWND>,
    renderer: Option<Renderer>,
    font_library: FontLibrary,
    font_entry: Option<Arc<FontEntry>>,
    font_family: String,
    font_request: String,
    wallpaper: Option<Wallpaper>,
    wallpaper_path: Option<PathBuf>,
    monitor_rect: [i32; 4],
    window_rect: [i32; 4],
    backdrop_settle: Option<Instant>,
    layout_dirty: bool,
    last_background_ms: f64,
    first_background_ms: f64,
    last_backdrop_ms: f64,
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
    peek_until: Option<Instant>,
    peek_restore_hidden: bool,
    pause_hide_at: Option<Instant>,
    position_settle: Option<Instant>,
    preview_probe_at: Option<Instant>,
    cover_retry_at: Option<Instant>,
    cover_retry_count: u8,
    fullscreen: bool,
    fullscreen_hidden: bool,
    fullscreen_started: bool,
    quit_at: Option<Instant>,
    trim_at: Option<Instant>,
    fetching_key: Option<String>,
    lyrics_retry_at: Option<Instant>,
    lyrics_retry_count: u8,
    config_fade: Option<ConfigFade>,
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
        let status_shared = Arc::new(Mutex::new(IpcStatus::default()));
        let config_path = options
            .config_path
            .clone()
            .unwrap_or_else(|| paths.config.clone());
        let quit_at = options.quit_after.map(|delay| Instant::now() + delay);
        let font_library = FontLibrary::new(stock_font_dir(), paths.fonts.clone());
        Self {
            options,
            config,
            paths,
            config_path,
            proxy,
            config_shared: shared,
            status_shared,
            settings_open: false,
            settings_demo: false,
            ipc_started: false,
            window: None,
            surface: None,
            hwnd: None,
            renderer: None,
            font_library,
            font_entry: None,
            font_family: "Lato".to_string(),
            font_request: String::new(),
            wallpaper: None,
            wallpaper_path: None,
            monitor_rect: [0, 0, 0, 0],
            window_rect: [0, 0, 0, 0],
            backdrop_settle: None,
            layout_dirty: false,
            last_background_ms: 0.0,
            first_background_ms: 0.0,
            last_backdrop_ms: 0.0,
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
            peek_until: None,
            peek_restore_hidden: false,
            pause_hide_at: None,
            position_settle: None,
            preview_probe_at: None,
            cover_retry_at: None,
            cover_retry_count: 0,
            fullscreen: false,
            fullscreen_hidden: false,
            fullscreen_started: false,
            quit_at,
            trim_at: None,
            fetching_key: None,
            lyrics_retry_at: None,
            lyrics_retry_count: 0,
            config_fade: None,
            bench: None,
        }
    }

    fn effective(&self) -> Config {
        self.config.effective()
    }

    fn begin_config_fade(&mut self) {
        let effective = self.effective();
        if effective.behavior.reduce_motion || effective.behavior.battery_saver {
            return;
        }
        let duration =
            Duration::from_millis(self.config.behavior.config_transition_ms.max(1) as u64);
        if let Some(renderer) = &self.renderer {
            if let Some(metrics) = &renderer.metrics {
                let frame = renderer.frame();
                if !frame.is_empty() {
                    self.config_fade = Some(ConfigFade {
                        old: frame.to_vec(),
                        width: metrics.window_w.max(1) as usize,
                        height: metrics.window_h.max(1) as usize,
                        started: Instant::now(),
                        duration,
                    });
                }
            }
        }
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

    fn update_status(&self) {
        let track = self.track.as_ref().map(|track| TrackInfo {
            key: track.key.clone(),
            title: track.title.clone(),
            artist: track.artist.clone(),
            album: track.album.clone(),
            source_app: track.source_app.clone(),
        });
        let line = self.lyrics.as_ref().and_then(|lyrics| {
            self.current_index
                .and_then(|index| lyrics.lines.get(index))
                .map(|line| line.text.clone())
        });
        let (lyrics_status, lyrics_message) = match &self.content {
            Content::Synced => ("synced".to_string(), None),
            Content::Plain => ("plain".to_string(), None),
            Content::Instrumental => ("instrumental".to_string(), None),
            Content::Loading => ("loading".to_string(), None),
            Content::NoLyrics => ("not_found".to_string(), None),
            Content::Error(message) => ("error".to_string(), Some(message.clone())),
            Content::Empty => ("empty".to_string(), None),
        };
        if let Ok(mut status) = self.status_shared.lock() {
            *status = IpcStatus {
                track,
                playback: self.playback.clone(),
                line,
                line_index: self.current_index,
                lyrics_status,
                lyrics_message,
            };
        }
    }

    fn apply_config(&mut self, config: Config) {
        let previous = std::mem::replace(&mut self.config, config);
        self.config.validate();
        let previous_effective = previous.effective();
        let effective = self.effective();
        if let Ok(mut shared) = self.config_shared.lock() {
            *shared = self.config.clone();
        }
        if self.config.behavior.click_through != previous.behavior.click_through {
            if let Some(hwnd) = self.hwnd {
                self.style = Some(window::set_click_through(
                    hwnd,
                    self.config.behavior.click_through,
                ));
            }
        }
        if self.config.hotkeys != previous.hotkeys {
            if let Some(tray) = &mut self.tray {
                tray.set_hotkeys(&self.config.hotkeys);
            }
        }
        if self.config.behavior.start_with_windows != previous.behavior.start_with_windows {
            crate::autostart::apply(self.config.behavior.start_with_windows);
        }
        if self.config.behavior.source != previous.behavior.source
            || self.config.behavior.source_custom != previous.behavior.source_custom
            || self.config.behavior.preferred_apps != previous.behavior.preferred_apps
            || self.config.behavior.ignored_apps != previous.behavior.ignored_apps
        {
            if let Some(media) = &self.media {
                media.request_track();
            }
        }
        if self.config.layout.scale != previous.layout.scale {
            self.scale = Scale::new(self.scale.os, self.config.layout.scale);
        }
        if self.config.layout != previous.layout || self.config.mode != previous.mode {
            self.layout_dirty = true;
        }
        let globals_changed = self.config.layout.anchor != previous.layout.anchor
            || self.config.layout.offset_x != previous.layout.offset_x
            || self.config.layout.offset_y != previous.layout.offset_y
            || self.config.layout.scale != previous.layout.scale
            || self.config.layout.width != previous.layout.width;
        let mut cleared_positions = false;
        if globals_changed && !self.config.layout.per_monitor.is_empty() {
            self.config.layout.per_monitor.clear();
            cleared_positions = true;
        }
        if self.config.lyrics != previous.lyrics || self.config.per_song != previous.per_song {
            self.recompute_index(false);
        }
        self.update_timers();
        self.schedule_pause_hide();
        if cleared_positions {
            self.persist_config();
        }
        let background_changed = background_visuals_changed(&effective, &previous_effective);
        let text_changed = text_visuals_changed(&effective, &previous_effective);
        if background_changed || text_changed {
            self.begin_config_fade();
        }
        if background_changed {
            self.background_dirty = true;
        }
        if text_changed {
            self.text_dirty = true;
        }
        self.update_status();
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn handle_settings_open(&mut self, event_loop: &ActiveEventLoop, open: bool) {
        self.settings_open = open;
        if open {
            if self.track.is_none() {
                self.settings_demo = true;
                self.seed_demo();
            } else {
                self.settings_demo = false;
            }
            if let Some(media) = &self.media {
                media.request_track();
            }
            self.preview_probe_at = if self.settings_demo {
                Some(Instant::now() + Duration::from_millis(1200))
            } else {
                None
            };
            self.user_hidden = false;
            self.pause_hide_at = None;
            self.position_settle = None;
            self.fullscreen_hidden = false;
            if let Err(error) = self.ensure_window(event_loop) {
                eprintln!("iceberg: settings preview window failed: {error}");
                return;
            }
            if let Some(hwnd) = self.hwnd {
                window::show_no_activate(hwnd);
            }
            self.background_dirty = true;
            self.text_dirty = true;
            self.update_timers();
            self.update_status();
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        } else {
            self.preview_probe_at = None;
            if self.settings_demo {
                self.settings_demo = false;
                self.track = None;
                self.lyrics = None;
                self.art = None;
                self.content = Content::Empty;
                self.current_index = None;
                self.transition = None;
                self.transition_end = None;
                self.fetching_key = None;
                self.next_line = None;
                self.next_progress = None;
            }
            self.update_status();
            if self.track.is_none() && !self.options.demo && self.options.bench.is_none() {
                self.destroy_window();
            } else if self.fullscreen
                && self.config.behavior.hide_on_fullscreen
                && !self.user_hidden
            {
                self.fullscreen_hidden = true;
                self.pause_hide_at = None;
                self.next_line = None;
                self.next_progress = None;
                self.next_frame = None;
                self.transition = None;
                self.transition_end = None;
                self.backdrop_settle = None;
                if let Some(hwnd) = self.hwnd {
                    window::hide(hwnd);
                }
                self.trim_at = Some(Instant::now() + Duration::from_millis(300));
            } else {
                self.schedule_pause_hide();
                self.update_timers();
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
        }
    }

    fn handle_show_widget(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            if self.track.is_none() {
                self.settings_demo = true;
                self.seed_demo();
            }
            if let Err(error) = self.ensure_window(event_loop) {
                eprintln!("iceberg: could not show the preview: {error}");
                return;
            }
        }
        self.user_hidden = false;
        if let Some(hwnd) = self.hwnd {
            window::show_no_activate(hwnd);
        }
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn handle_fonts_changed(&mut self) {
        self.font_library.clear();
        self.font_entry = None;
        self.font_request.clear();
        self.background_dirty = true;
        self.text_dirty = true;
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn refresh_lyrics(&mut self) {
        let Some(track) = &self.track else {
            return;
        };
        let track = track.clone();
        self.fetching_key = None;
        self.lyrics = None;
        self.content = Content::Loading;
        self.current_index = None;
        self.transition = None;
        self.transition_end = None;
        self.lyrics_retry_at = None;
        self.lyrics_retry_count = 0;
        self.text_dirty = true;
        self.start_lyrics_fetch(&track, true);
        self.update_timers();
        self.update_status();
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn handle_fullscreen(&mut self, event_loop: &ActiveEventLoop, fullscreen: bool) {
        self.fullscreen = fullscreen;
        if !self.config.behavior.hide_on_fullscreen {
            if self.fullscreen_hidden {
                self.show_after_fullscreen(event_loop);
            }
            return;
        }
        if fullscreen {
            if self.settings_open || self.user_hidden || self.window.is_none() {
                return;
            }
            self.fullscreen_hidden = true;
            self.pause_hide_at = None;
            self.next_line = None;
            self.next_progress = None;
            self.next_frame = None;
            self.transition = None;
            self.transition_end = None;
            self.backdrop_settle = None;
            if let Some(hwnd) = self.hwnd {
                window::hide(hwnd);
            }
            self.trim_at = Some(Instant::now() + Duration::from_millis(300));
        } else if self.fullscreen_hidden {
            self.show_after_fullscreen(event_loop);
        }
    }

    fn show_after_fullscreen(&mut self, event_loop: &ActiveEventLoop) {
        self.fullscreen_hidden = false;
        if self.user_hidden || self.settings_open {
            return;
        }
        if self.window.is_none() {
            if self.track.is_none() {
                return;
            }
            if let Err(error) = self.ensure_window(event_loop) {
                eprintln!("iceberg: could not restore the widget after fullscreen: {error}");
                return;
            }
        }
        if let Some(hwnd) = self.hwnd {
            window::show_no_activate(hwnd);
        }
        self.background_dirty = true;
        self.text_dirty = true;
        self.recompute_index(false);
        self.update_timers();
        self.update_status();
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn persist_config(&mut self) {
        self.config.validate();
        if let Err(error) = self.config.save(&self.config_path) {
            eprintln!("iceberg: could not save the config: {error}");
        }
        if let Ok(mut shared) = self.config_shared.lock() {
            *shared = self.config.clone();
        }
    }

    fn toggle_click_through(&mut self) {
        self.config.behavior.click_through = !self.config.behavior.click_through;
        if let Some(hwnd) = self.hwnd {
            self.style = Some(window::set_click_through(
                hwnd,
                self.config.behavior.click_through,
            ));
        }
        self.persist_config();
    }

    fn cycle_mode(&mut self) {
        self.config.mode = match self.config.mode {
            WidgetMode::Card => WidgetMode::Player,
            WidgetMode::Player => WidgetMode::Lyrics,
            WidgetMode::Lyrics => WidgetMode::Island,
            WidgetMode::Island => WidgetMode::Card,
        };
        self.persist_config();
        self.background_dirty = true;
        self.text_dirty = true;
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn adjust_current_offset(&mut self, delta_ms: i64) {
        let Some(track) = &self.track else {
            return;
        };
        let current = self.config.per_song_offset(&track.key);
        self.config.set_song_offset(&track.key, current + delta_ms);
        self.persist_config();
        self.recompute_index(false);
        self.update_timers();
        self.update_status();
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn start_peek(&mut self, event_loop: &ActiveEventLoop, duration: Duration) {
        let restore = self.user_hidden || self.window.is_none();
        self.peek_restore_hidden = restore;
        if restore {
            self.set_visibility(event_loop, true);
        }
        self.peek_until = Some(Instant::now() + duration);
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn set_visibility(&mut self, event_loop: &ActiveEventLoop, visible: bool) {
        if visible {
            self.user_hidden = false;
            self.pause_hide_at = None;
            self.fullscreen_hidden = false;
            if let Err(error) = self.ensure_window(event_loop) {
                eprintln!("iceberg: could not show the widget: {error}");
                return;
            }
            if let Some(hwnd) = self.hwnd {
                window::show_no_activate(hwnd);
            }
            let effective = self.effective();
            self.ensure_wallpaper(&effective);
            self.background_dirty = true;
            self.recompute_index(false);
            self.update_timers();
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        } else {
            self.user_hidden = true;
            self.peek_until = None;
            self.peek_restore_hidden = false;
            self.next_line = None;
            self.next_progress = None;
            self.next_frame = None;
            self.transition = None;
            self.transition_end = None;
            self.backdrop_settle = None;
            if let Some(hwnd) = self.hwnd {
                window::hide(hwnd);
            }
            self.trim_at = Some(Instant::now() + Duration::from_millis(300));
        }
    }

    fn launch_settings(&self) {
        let Ok(exe) = std::env::current_exe() else {
            return;
        };
        let Some(directory) = exe.parent() else {
            return;
        };
        let settings = directory.join("iceberg-settings.exe");
        if let Err(error) = std::process::Command::new(&settings).spawn() {
            eprintln!("iceberg: could not launch {}: {error}", settings.display());
        }
    }

    fn schedule_pause_hide(&mut self) {
        if self.settings_open
            || self.user_hidden
            || self.window.is_none()
            || self.playback.status != PlaybackStatus::Paused
        {
            self.pause_hide_at = None;
            return;
        }
        let seconds = self.config.behavior.pause_hide_seconds;
        self.pause_hide_at =
            (seconds > 0).then(|| Instant::now() + Duration::from_secs(u64::from(seconds)));
    }

    fn show_for_playback(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() || self.user_hidden || self.settings_open || self.fullscreen_hidden
        {
            return;
        }
        self.fullscreen = crate::fullscreen::foreground_is_fullscreen();
        if self.fullscreen && self.config.behavior.hide_on_fullscreen {
            self.fullscreen_hidden = true;
            return;
        }
        if let Err(error) = self.ensure_window(event_loop) {
            eprintln!("iceberg: could not recreate the widget window: {error}");
            return;
        }
        if let Some(hwnd) = self.hwnd {
            window::show_no_activate(hwnd);
        }
        self.background_dirty = true;
        self.text_dirty = true;
        self.update_timers();
        self.update_status();
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn settle_window_position(&mut self) {
        if self.user_hidden || self.fullscreen_hidden {
            return;
        }
        let (Some(hwnd), Some(metrics)) = (
            self.hwnd,
            self.renderer
                .as_ref()
                .and_then(|renderer| renderer.metrics.clone()),
        ) else {
            return;
        };
        let width = self.window_rect[2].max(1);
        let height = self.window_rect[3].max(1);
        let work = window::work_area(hwnd);
        let mut x = self.window_rect[0];
        let mut y = self.window_rect[1];
        if self.config.behavior.edge_snap {
            let (next_x, next_y) = window::snap_position(work, x, y, width, height, 16);
            if (next_x, next_y) != (x, y) {
                window::place(hwnd, next_x, next_y, width, height);
                x = next_x;
                y = next_y;
                self.background_dirty = true;
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
        }
        self.window_rect[0] = x;
        self.window_rect[1] = y;
        if self.monitor_name.is_empty() {
            return;
        }
        let scale = self.scale.effective();
        let card_w = metrics.card_w.round() as i32;
        let card_h = metrics.card_h.round() as i32;
        let margin = metrics.margin.round() as i32;
        let anchors = [
            Anchor::TopLeft,
            Anchor::TopCenter,
            Anchor::TopRight,
            Anchor::CenterLeft,
            Anchor::Center,
            Anchor::CenterRight,
            Anchor::BottomLeft,
            Anchor::BottomCenter,
            Anchor::BottomRight,
        ];
        let mut best: Option<(Anchor, i32, i32, i64)> = None;
        for anchor in anchors {
            let (base_x, base_y) = window::anchored_position(
                work, width, height, card_w, card_h, margin, anchor, 0, 0,
            );
            let distance = i64::from(x - base_x).pow(2) + i64::from(y - base_y).pow(2);
            if best
                .map(|(_, _, _, best_distance)| distance < best_distance)
                .unwrap_or(true)
            {
                let offset_x = ((x - base_x) as f64 / scale).round() as i32;
                let offset_y = ((y - base_y) as f64 / scale).round() as i32;
                best = Some((anchor, offset_x, offset_y, distance));
            }
        }
        let Some((anchor, offset_x, offset_y, _)) = best else {
            return;
        };
        let layout = MonitorLayout {
            anchor,
            offset_x,
            offset_y,
        };
        if self.config.layout.per_monitor.get(&self.monitor_name) != Some(&layout) {
            self.config
                .layout
                .per_monitor
                .insert(self.monitor_name.clone(), layout);
            self.persist_config();
        }
    }

    fn ensure_wallpaper(&mut self, effective: &Config) {
        if !needs_wallpaper(effective.background.kind) {
            self.wallpaper = None;
            self.wallpaper_path = None;
            return;
        }
        let path = wallpaper_path();
        if path.is_none() {
            self.wallpaper = None;
            self.wallpaper_path = None;
            return;
        }
        if path == self.wallpaper_path && self.wallpaper.is_some() {
            return;
        }
        self.wallpaper_path = path.clone();
        self.wallpaper = path.as_ref().and_then(|value| Wallpaper::load_from(value));
    }

    fn build_backdrop(&mut self, effective: &Config, metrics: &Metrics) -> Option<BackdropCrop> {
        if !needs_wallpaper(effective.background.kind) {
            return None;
        }
        self.ensure_wallpaper(effective);
        let wallpaper = self.wallpaper.as_ref()?;
        let width = metrics.window_w.max(1) as usize;
        let height = metrics.window_h.max(1) as usize;
        Some(wallpaper.crop_cover(
            self.monitor_rect,
            [
                self.window_rect[0],
                self.window_rect[1],
                metrics.window_w.max(1),
                metrics.window_h.max(1),
            ],
            width,
            height,
        ))
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
        let monitor_rect = window
            .current_monitor()
            .map(|monitor| {
                let position = monitor.position();
                let size = monitor.size();
                [
                    position.x,
                    position.y,
                    size.width as i32,
                    size.height as i32,
                ]
            })
            .unwrap_or([
                work.left,
                work.top,
                work.right - work.left,
                work.bottom - work.top,
            ]);
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
        self.monitor_rect = monitor_rect;
        self.window_rect = [x, y, metrics.window_w.max(1), metrics.window_h.max(1)];
        self.backdrop_settle = None;
        self.ensure_wallpaper(&effective);
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
        self.wallpaper = None;
        self.wallpaper_path = None;
        self.backdrop_settle = None;
        self.position_settle = None;
        self.pause_hide_at = None;
        self.preview_probe_at = None;
        self.cover_retry_at = None;
        self.cover_retry_count = 0;
        self.fullscreen_hidden = false;
        self.last_background_ms = 0.0;
        self.first_background_ms = 0.0;
        self.last_backdrop_ms = 0.0;
        self.monitor_rect = [0, 0, 0, 0];
        self.window_rect = [0, 0, 0, 0];
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
        self.update_status();
        self.update_timers();
    }

    fn on_track(&mut self, track: Track, playback: Playback) {
        self.settings_demo = false;
        self.preview_probe_at = None;
        let same_track = self
            .track
            .as_ref()
            .map(|current| current.key == track.key)
            .unwrap_or(false);
        if !same_track {
            self.begin_config_fade();
        }
        if same_track {
            self.playback = playback;
            let missing_cover = self
                .track
                .as_ref()
                .map(|current| current.cover_png.is_none())
                .unwrap_or(false);
            if missing_cover {
                if track.cover_png.is_some() {
                    self.begin_config_fade();
                }
                if let Some(cover) = track.cover_png {
                    if let Some(current) = &mut self.track {
                        current.cover_png = Some(cover);
                        current.accent = track.accent;
                    }
                    self.art = None;
                    self.refresh_art();
                    self.background_dirty = true;
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
            }
            if self
                .track
                .as_ref()
                .map(|current| current.cover_png.is_some())
                .unwrap_or(false)
            {
                self.cover_retry_at = None;
                self.cover_retry_count = 0;
            } else {
                self.cover_retry_at = Some(Instant::now() + Duration::from_millis(900));
            }
            self.recompute_index(true);
            self.update_timers();
            self.update_status();
            if let Some(window) = &self.window {
                window.request_redraw();
            }
            return;
        }
        self.track = Some(track.clone());
        self.playback = playback;
        self.lyrics = None;
        self.content = Content::Loading;
        self.current_index = None;
        self.transition = None;
        self.transition_end = None;
        self.art = None;
        self.lyrics_retry_at = None;
        self.lyrics_retry_count = 0;
        self.cover_retry_count = 0;
        self.cover_retry_at = if track.cover_png.is_none() {
            Some(Instant::now() + Duration::from_millis(900))
        } else {
            None
        };
        self.refresh_art();
        self.background_dirty = true;
        self.text_dirty = true;
        self.fetching_key = None;
        let effective = self.effective();
        self.resolve_active_font(&effective);
        self.start_lyrics_fetch(&track, false);
        self.recompute_index(false);
        self.update_timers();
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn start_lyrics_fetch(&mut self, track: &Track, force: bool) {
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
                let lyrics = service.fetch_for(&track, force);
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
        if lyrics.kind == LyricsKind::Error {
            if self.lyrics_retry_count < 3 {
                let delay = match self.lyrics_retry_count {
                    0 => 10,
                    1 => 30,
                    _ => 60,
                };
                self.lyrics_retry_count += 1;
                self.lyrics_retry_at = Some(Instant::now() + Duration::from_secs(delay));
            } else {
                self.lyrics_retry_at = None;
            }
        } else {
            self.lyrics_retry_count = 0;
            self.lyrics_retry_at = None;
        }
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
            self.update_status();
            return;
        };
        let Some(lyrics) = &self.lyrics else {
            self.current_index = None;
            self.update_status();
            return;
        };
        if lyrics.kind != LyricsKind::Synced || lyrics.lines.is_empty() {
            self.current_index = None;
            self.update_status();
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
            if self
                .transition
                .map(|transition| transition.to.round() as usize == index)
                .unwrap_or(false)
            {
                self.update_status();
                self.update_timers();
                return;
            }
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
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }
        self.update_status();
        self.update_timers();
    }

    fn update_timers(&mut self) {
        self.schedule_line_deadline();
        self.schedule_progress();
    }

    fn schedule_line_deadline(&mut self) {
        self.next_line = None;
        // While a line transition is in flight, the transition-end handler owns the next
        // update. Rescheduling based on the still-old current_index here restarts the same
        // transition every frame and can freeze the active lyric.
        if self.transition.is_some() {
            return;
        }
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
            || !matches!(effective.mode, WidgetMode::Card | WidgetMode::Player)
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
        let size_changed = self
            .renderer
            .as_ref()
            .and_then(|renderer| renderer.metrics.as_ref())
            != Some(&metrics);
        if size_changed {
            if let Some(window) = &self.window {
                if let Ok(position) = window.outer_position() {
                    self.window_rect[0] = position.x;
                    self.window_rect[1] = position.y;
                }
            }
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
        self.window_rect[2] = metrics.window_w.max(1);
        self.window_rect[3] = metrics.window_h.max(1);
        if (size_changed || self.layout_dirty) && !self.user_hidden {
            if let Some(hwnd) = self.hwnd {
                let work = window::work_area(hwnd);
                let (anchor, offset_x, offset_y) = monitor_layout(&effective, &self.monitor_name);
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
                window::place(hwnd, x, y, metrics.window_w, metrics.window_h);
                self.window_rect = [x, y, metrics.window_w, metrics.window_h];
            }
            self.layout_dirty = false;
        }
        let background_started = Instant::now();
        let backdrop = if self.background_dirty {
            self.build_backdrop(&effective, &metrics)
        } else {
            None
        };
        let fade = self.config_fade.take();
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };
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
            backdrop: backdrop.as_ref(),
        };
        renderer.ensure_size(&view);
        if self.background_dirty {
            renderer.rebuild_background(&view);
            self.background_dirty = false;
            let elapsed_ms = background_started.elapsed().as_secs_f64() * 1000.0;
            self.last_background_ms = elapsed_ms;
            if self.first_background_ms == 0.0 {
                self.first_background_ms = elapsed_ms;
            }
            self.last_backdrop_ms = renderer.last_backdrop_ms;
        }
        if self.text_dirty {
            renderer.clear_text_cache();
            self.text_dirty = false;
        }
        renderer.render(&view);
        let new_w = metrics.window_w.max(1) as usize;
        let new_h = metrics.window_h.max(1) as usize;
        let mut blended = None;
        if let Some(fade) = fade {
            let elapsed = fade.started.elapsed().as_secs_f32();
            let progress = (elapsed / fade.duration.as_secs_f32()).clamp(0.0, 1.0);
            if progress < 1.0 {
                blended = Some(compose_config_fade(
                    renderer.frame(),
                    new_w,
                    new_h,
                    &fade,
                    progress,
                ));
                self.config_fade = Some(fade);
            }
        }
        if let (Some(surface), Some(hwnd)) = (&mut self.surface, self.hwnd) {
            let frame = blended.as_deref().unwrap_or_else(|| renderer.frame());
            if surface.pixels_mut().len() == frame.len() {
                surface.pixels_mut().copy_from_slice(frame);
                surface.present(hwnd);
            }
        }
        self.update_status();
    }

    fn handle_media(&mut self, event_loop: &ActiveEventLoop, event: MediaEvent) {
        match event {
            MediaEvent::Track { track, playback } => {
                self.fullscreen = crate::fullscreen::foreground_is_fullscreen();
                if self.fullscreen && self.config.behavior.hide_on_fullscreen && !self.settings_open
                {
                    self.fullscreen_hidden = true;
                    if let Some(hwnd) = self.hwnd {
                        window::hide(hwnd);
                    }
                    self.on_track(track, playback);
                    self.schedule_pause_hide();
                    return;
                }
                self.on_track(track, playback);
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
                self.schedule_pause_hide();
            }
            MediaEvent::Playback(playback) => {
                self.playback = playback;
                self.recompute_index(true);
                self.update_timers();
                if self.track.is_none() {
                    if let Some(media) = &self.media {
                        media.request_track();
                    }
                }
                if self.playback.status == PlaybackStatus::Playing {
                    self.show_for_playback(event_loop);
                }
                self.schedule_pause_hide();
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            MediaEvent::Stopped | MediaEvent::NoSession => {
                self.pause_hide_at = None;
                self.position_settle = None;
                if self.settings_open {
                    self.settings_demo = true;
                    self.seed_demo();
                    self.preview_probe_at = Some(Instant::now() + Duration::from_millis(1200));
                    self.update_timers();
                    return;
                }
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
                self.update_status();
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
                let visible = self.user_hidden || self.window.is_none();
                self.set_visibility(event_loop, visible);
            }
            TrayAction::Settings => self.launch_settings(),
            TrayAction::ClickThrough => self.toggle_click_through(),
            TrayAction::CycleMode => self.cycle_mode(),
            TrayAction::Hotkey(action) => match action {
                HotkeyAction::Toggle => {
                    let visible = self.user_hidden || self.window.is_none();
                    self.set_visibility(event_loop, visible);
                }
                HotkeyAction::ClickThrough => self.toggle_click_through(),
                HotkeyAction::OffsetUp => self.adjust_current_offset(100),
                HotkeyAction::OffsetDown => self.adjust_current_offset(-100),
                HotkeyAction::Mode => self.cycle_mode(),
                HotkeyAction::Peek => {
                    self.start_peek(event_loop, Duration::from_secs(3));
                }
                HotkeyAction::Settings => self.launch_settings(),
            },
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
                        bench.background_start = self
                            .renderer
                            .as_ref()
                            .map(|renderer| renderer.background_rebuilds)
                            .unwrap_or(0);
                        self.background_dirty = true;
                        if let Some(window) = &self.window {
                            window.request_redraw();
                        }
                        bench.stage = 10;
                        bench.next_transition = now + Duration::from_millis(400);
                        event_loop.set_control_flow(ControlFlow::WaitUntil(
                            now + Duration::from_millis(16),
                        ));
                    } else {
                        event_loop.set_control_flow(ControlFlow::WaitUntil(
                            bench.started + Duration::from_secs_f64(1.0),
                        ));
                    }
                }
                10 => {
                    let count = self
                        .renderer
                        .as_ref()
                        .map(|renderer| renderer.background_rebuilds)
                        .unwrap_or(0);
                    if count > bench.background_start || now >= bench.next_transition {
                        bench.transition_cpu_start = Some(perf::cpu_times());
                        bench.transition_started = now;
                        bench.background_start = count;
                        bench.stage = 1;
                        bench.next_transition = now;
                        event_loop.set_control_flow(ControlFlow::WaitUntil(now));
                    } else {
                        event_loop.set_control_flow(ControlFlow::WaitUntil(
                            now + Duration::from_millis(16),
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
                    let stage_end = bench.transition_started + Duration::from_secs_f64(2.9);
                    next = next.min(stage_end);
                    if now >= stage_end {
                        bench.stage = 20;
                        bench.transition_cpu_end = Some(perf::cpu_times());
                        bench.background_end = self
                            .renderer
                            .as_ref()
                            .map(|renderer| renderer.background_rebuilds)
                            .unwrap_or(0);
                        self.transition = None;
                        self.transition_end = None;
                        self.next_frame = None;
                        if let Some(window) = &self.window {
                            window.request_redraw();
                        }
                        bench.next_transition = now + Duration::from_millis(200);
                        event_loop.set_control_flow(ControlFlow::WaitUntil(bench.next_transition));
                    } else {
                        event_loop.set_control_flow(ControlFlow::WaitUntil(next));
                    }
                }
                20 => {
                    if now >= bench.next_transition {
                        bench.idle_cpu_start = Some(perf::cpu_times());
                        bench.idle_start = now;
                        bench.idle_end = now + Duration::from_secs(2);
                        bench.stage = 2;
                        event_loop.set_control_flow(ControlFlow::WaitUntil(bench.idle_end));
                    } else {
                        event_loop.set_control_flow(ControlFlow::WaitUntil(bench.next_transition));
                    }
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
        let effective = self.effective();
        let background = background_label(effective.background.kind);
        let backdrop = if !needs_wallpaper(effective.background.kind) {
            "none"
        } else if self.wallpaper.is_some() {
            "wallpaper"
        } else {
            "tint"
        };
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
            background,
            backdrop,
            background_build_ms: self.last_background_ms,
            first_background_ms: self.first_background_ms,
            backdrop_pipeline_ms: self.last_backdrop_ms,
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
        if self.options.bench.is_none() && !self.ipc_started {
            self.ipc_started = true;
            crate::ipc_server::start(
                self.proxy.clone(),
                self.config_shared.clone(),
                self.status_shared.clone(),
                self.config_path.clone(),
            );
        }
        if self.options.bench.is_none() && !self.fullscreen_started {
            self.fullscreen_started = true;
            self.fullscreen = crate::fullscreen::foreground_is_fullscreen();
            crate::fullscreen::start(self.proxy.clone());
        }
        if self.options.bench.is_none() {
            crate::autostart::apply(self.config.behavior.start_with_windows);
        }
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
                Ok(mut tray) => {
                    tray.set_hotkeys(&self.config.hotkeys);
                    self.tray = Some(tray);
                }
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
                transition_started: start,
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
            WindowEvent::Moved(position) => {
                let mut monitor_changed = false;
                if let Some(window) = &self.window {
                    self.window_rect[0] = position.x;
                    self.window_rect[1] = position.y;
                    let name = window
                        .current_monitor()
                        .and_then(|monitor| monitor.name())
                        .unwrap_or_default();
                    if let Some(monitor) = window.current_monitor() {
                        let origin = monitor.position();
                        let size = monitor.size();
                        self.monitor_rect =
                            [origin.x, origin.y, size.width as i32, size.height as i32];
                    }
                    if name != self.monitor_name {
                        self.monitor_name = name;
                        monitor_changed = true;
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
                let effective = self.effective();
                self.ensure_wallpaper(&effective);
                self.position_settle = Some(Instant::now() + Duration::from_millis(160));
                if needs_wallpaper(effective.background.kind) {
                    self.backdrop_settle = Some(Instant::now() + Duration::from_millis(120));
                    if monitor_changed {
                        self.background_dirty = true;
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
            AppEvent::Config(config) => {
                self.apply_config(config);
                if !self.config.behavior.hide_on_fullscreen && self.fullscreen_hidden {
                    self.show_after_fullscreen(event_loop);
                }
            }
            AppEvent::SettingsOpen(open) => self.handle_settings_open(event_loop, open),
            AppEvent::ShowWidget => self.handle_show_widget(event_loop),
            AppEvent::FontsChanged => self.handle_fonts_changed(),
            AppEvent::RefreshLyrics => self.refresh_lyrics(),
            AppEvent::Visibility(visible) => self.set_visibility(event_loop, visible),
            AppEvent::Fullscreen(fullscreen) => self.handle_fullscreen(event_loop, fullscreen),
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
        if let Some(deadline) = self.pause_hide_at {
            if now >= deadline {
                self.pause_hide_at = None;
                if self.playback.status == PlaybackStatus::Paused
                    && !self.settings_open
                    && !self.user_hidden
                {
                    self.destroy_window();
                }
            } else {
                next = Some(next.map(|value| value.min(deadline)).unwrap_or(deadline));
            }
        }
        if let Some(deadline) = self.cover_retry_at {
            if now >= deadline {
                self.cover_retry_at = None;
                let missing = self
                    .track
                    .as_ref()
                    .map(|track| track.cover_png.is_none())
                    .unwrap_or(false);
                if missing && self.cover_retry_count < 3 {
                    self.cover_retry_count += 1;
                    if let Some(media) = &self.media {
                        media.request_track();
                    }
                    self.cover_retry_at = Some(now + Duration::from_millis(1000));
                }
            } else {
                next = Some(next.map(|value| value.min(deadline)).unwrap_or(deadline));
            }
        }
        if let Some(deadline) = self.lyrics_retry_at {
            if now >= deadline {
                self.lyrics_retry_at = None;
                if let Some(track) = self.track.clone() {
                    self.fetching_key = None;
                    self.content = Content::Loading;
                    self.text_dirty = true;
                    self.start_lyrics_fetch(&track, true);
                    self.update_status();
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
            } else {
                next = Some(next.map(|value| value.min(deadline)).unwrap_or(deadline));
            }
        }
        if let Some(fade) = &self.config_fade {
            let end = fade.started + fade.duration;
            if now >= end {
                self.config_fade = None;
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            } else {
                let frame = now + Duration::from_millis(16);
                next = Some(next.map(|value| value.min(frame)).unwrap_or(frame));
                next = Some(next.map(|value| value.min(end)).unwrap_or(end));
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
        }
        if let Some(deadline) = self.preview_probe_at {
            if now >= deadline {
                self.preview_probe_at = None;
                if self.settings_open && self.settings_demo {
                    if let Some(media) = &self.media {
                        media.request_track();
                    }
                    self.preview_probe_at = Some(now + Duration::from_millis(1500));
                }
            } else {
                next = Some(next.map(|value| value.min(deadline)).unwrap_or(deadline));
            }
        }
        if let Some(deadline) = self.peek_until {
            if now >= deadline {
                self.peek_until = None;
                if self.peek_restore_hidden {
                    self.peek_restore_hidden = false;
                    self.set_visibility(event_loop, false);
                    if self.track.is_none() && !self.options.demo && self.options.bench.is_none() {
                        self.destroy_window();
                    }
                }
            } else {
                next = Some(next.map(|value| value.min(deadline)).unwrap_or(deadline));
            }
        }
        if let Some(deadline) = self.backdrop_settle {
            if now >= deadline {
                self.backdrop_settle = None;
                self.background_dirty = true;
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            } else {
                next = Some(next.map(|value| value.min(deadline)).unwrap_or(deadline));
            }
        }
        if let Some(deadline) = self.position_settle {
            if now >= deadline {
                self.position_settle = None;
                self.settle_window_position();
            } else {
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

fn compose_config_fade(
    current: &[u8],
    width: usize,
    height: usize,
    fade: &ConfigFade,
    progress: f32,
) -> Vec<u8> {
    let expected = width.saturating_mul(height).saturating_mul(4);
    let old_expected = fade.width.saturating_mul(fade.height).saturating_mul(4);
    if current.len() != expected || fade.old.len() != old_expected || expected == 0 {
        return current.to_vec();
    }
    let progress = progress.clamp(0.0, 1.0);
    let t = progress * progress * (3.0 - 2.0 * progress);
    let mut output = vec![0u8; expected];
    for y in 0..height {
        let old_y = (y * fade.height / height).min(fade.height.saturating_sub(1));
        for x in 0..width {
            let old_x = (x * fade.width / width).min(fade.width.saturating_sub(1));
            let new_index = (y * width + x) * 4;
            let old_index = (old_y * fade.width + old_x) * 4;
            for channel in 0..4 {
                output[new_index + channel] = (current[new_index + channel] as f32 * t
                    + fade.old[old_index + channel] as f32 * (1.0 - t))
                    .round() as u8;
            }
        }
    }
    output
}

fn background_visuals_changed(current: &Config, previous: &Config) -> bool {
    let a = &current.typography;
    let b = &previous.typography;
    current.background != previous.background
        || current.mode != previous.mode
        || current.layout != previous.layout
        || current.behavior.show_progress != previous.behavior.show_progress
        || current.behavior.show_art != previous.behavior.show_art
        || current.behavior.show_meta != previous.behavior.show_meta
        || a.font_family != b.font_family
        || a.font_size != b.font_size
        || a.line_height != b.line_height
        || a.align != b.align
        || a.active_color != b.active_color
        || a.shadow != b.shadow
        || a.outline != b.outline
        || a.visible_lines != b.visible_lines
}

fn text_visuals_changed(current: &Config, previous: &Config) -> bool {
    let a = &current.typography;
    let b = &previous.typography;
    a.font_family != b.font_family
        || a.font_size != b.font_size
        || a.weight != b.weight
        || a.italic != b.italic
        || a.letter_spacing != b.letter_spacing
        || a.active_color != b.active_color
        || a.case != b.case
        || current.per_song != previous.per_song
        || current.lyrics.wrap != previous.lyrics.wrap
}

fn background_label(kind: BackgroundKind) -> &'static str {
    match kind {
        BackgroundKind::Solid => "solid",
        BackgroundKind::Transparent => "transparent",
        BackgroundKind::Acrylic => "acrylic",
        BackgroundKind::Ice => "ice",
        BackgroundKind::Liquid => "liquid",
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
