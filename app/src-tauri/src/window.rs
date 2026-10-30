use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;

use tauri::utils::config::WindowEffectsConfig;
use tauri::window::Effect;
use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder, WindowEvent,
};
use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2_3;
use webview2_com::TrySuspendCompletedHandler;
use windows::core::Interface;

use crate::config::Config;
use crate::state::{AppState, SavedPosition};
use crate::util::replace_file;

pub const WIDGET_LABEL: &str = "widget";
pub const SETTINGS_LABEL: &str = "settings";

const DEFAULT_WEBVIEW_ARGS: &str =
    "--disable-background-networking --disable-extensions --disable-sync \
--disable-default-apps --no-pings \
--disable-features=msSmartScreenProtection,msWebOOUI,msPdfOOUI,TranslateUI \
--renderer-process-limit=1 --js-flags=--max-old-space-size=32 --disk-cache-size=16777216 \
--single-process --disable-gpu";

fn webview_args() -> String {
    std::env::var("ICEBERG_WEBVIEW_ARGS").unwrap_or_else(|_| DEFAULT_WEBVIEW_ARGS.to_string())
}

const SNAP_THRESHOLD: f64 = 24.0;

pub fn widget_size(config: &Config, expanded: bool) -> (f64, f64) {
    let lines = config.appearance.lyric_lines.clamp(3, 5) as f64;
    let line_height = (config.appearance.font_size as f64 * 1.45 + 2.0).max(16.0);
    match config.mode.as_str() {
        "island" => {
            if expanded {
                (384.0, 86.0 + lines * line_height)
            } else {
                (384.0, 76.0)
            }
        }
        "lyrics" => (460.0, 88.0 + (lines - 1.0) * line_height),
        _ => (408.0, 92.0 + lines * line_height),
    }
}

fn effects_for(config: &Config) -> Option<WindowEffectsConfig> {
    match config.appearance.effect.as_str() {
        "acrylic" => Some(
            tauri::window::EffectsBuilder::new()
                .effect(Effect::Acrylic)
                .build(),
        ),
        "mica" => Some(
            tauri::window::EffectsBuilder::new()
                .effect(Effect::Mica)
                .build(),
        ),
        _ => None,
    }
}

pub fn ensure_widget(app: &AppHandle) -> Option<WebviewWindow> {
    if let Some(window) = app.get_webview_window(WIDGET_LABEL) {
        return Some(window);
    }
    create_widget(app)
}

fn create_widget(app: &AppHandle) -> Option<WebviewWindow> {
    let state = app.state::<AppState>();
    let config = state.config();
    let (width, height) = widget_size(&config, false);
    let data_dir = state.webview_dir.clone();
    let args = webview_args();

    let mut builder =
        WebviewWindowBuilder::new(app, WIDGET_LABEL, WebviewUrl::App("index.html".into()))
            .title("Iceberg")
            .inner_size(width, height)
            .resizable(false)
            .maximizable(false)
            .decorations(false)
            .transparent(true)
            .shadow(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .focused(false)
            .focusable(false)
            .visible(false)
            .additional_browser_args(&args)
            .data_directory(data_dir);

    if let Some(effects) = effects_for(&config) {
        builder = builder.effects(effects);
    }

    let window = match builder.build() {
        Ok(window) => window,
        Err(error) => {
            eprintln!("iceberg: could not create widget: {error}");
            return None;
        }
    };

    let _ = window.set_ignore_cursor_events(config.behavior.click_through);
    place_widget(&window);
    wire_widget_events(&window, app);
    Some(window)
}

fn place_widget(window: &WebviewWindow) {
    let state = window.app_handle().state::<AppState>();
    let config = state.config();
    let memory = state
        .window_memory
        .lock()
        .map(|memory| memory.clone())
        .unwrap_or_default();
    let monitors = window.available_monitors().unwrap_or_default();
    let primary = window.primary_monitor().ok().flatten();
    let target = monitors
        .iter()
        .find(|monitor| {
            monitor
                .name()
                .map(|name| name == &memory.last_monitor)
                .unwrap_or(false)
        })
        .or(primary.as_ref())
        .or_else(|| monitors.first());

    let Some(monitor) = target else {
        return;
    };

    let scale = monitor.scale_factor();
    let (logical_width, logical_height) = widget_size(&config, false);
    let width = (logical_width * scale).round() as i32;
    let height = (logical_height * scale).round() as i32;
    let work = monitor.work_area();
    let work_x = work.position.x;
    let work_y = work.position.y;
    let work_w = work.size.width as i32;
    let work_h = work.size.height as i32;

    let default_position = (
        work_x + (work_w - width).max(0) / 2,
        work_y + (24.0 * scale).round() as i32,
    );

    let position = monitor
        .name()
        .and_then(|name| memory.positions.get(name).copied())
        .map(|saved| {
            let visible_x = saved.x + width > work_x + 48 && saved.x < work_x + work_w - 48;
            let visible_y = saved.y + height > work_y + 24 && saved.y < work_y + work_h - 28;
            if visible_x && visible_y {
                (saved.x, saved.y)
            } else {
                default_position
            }
        })
        .unwrap_or(default_position);

    let _ = window.set_position(PhysicalPosition::new(position.0, position.1));
}

fn wire_widget_events(window: &WebviewWindow, app: &AppHandle) {
    let handle = app.clone();
    window.on_window_event(move |event| match event {
        WindowEvent::Moved(_) => schedule_position_save(&handle),
        WindowEvent::ScaleFactorChanged { .. } => schedule_position_save(&handle),
        WindowEvent::Destroyed => {
            let state = handle.state::<AppState>();
            state.widget_loaded.store(false, Ordering::SeqCst);
            state.widget_suspended.store(false, Ordering::SeqCst);
            state.island_expanded.store(false, Ordering::SeqCst);
        }
        _ => {}
    });
}

fn monitor_name(monitor: &tauri::Monitor) -> String {
    monitor
        .name()
        .cloned()
        .unwrap_or_else(|| format!("{}x{}", monitor.size().width, monitor.size().height))
}

fn schedule_position_save(app: &AppHandle) {
    let state = app.state::<AppState>();
    let generation = state.move_generation.fetch_add(1, Ordering::SeqCst) + 1;
    let handle = app.clone();
    thread::Builder::new()
        .name("iceberg-position".into())
        .spawn(move || {
            thread::sleep(Duration::from_millis(280));
            let state = handle.state::<AppState>();
            if state.move_generation.load(Ordering::SeqCst) != generation {
                return;
            }
            save_position(&handle);
        })
        .ok();
}

fn save_position(app: &AppHandle) {
    let state = app.state::<AppState>();
    let Some(window) = app.get_webview_window(WIDGET_LABEL) else {
        return;
    };
    let Ok(mut position) = window.outer_position() else {
        return;
    };
    let Ok(size) = window.outer_size() else {
        return;
    };
    let Some(monitor) = window.current_monitor().ok().flatten() else {
        return;
    };
    let config = state.config();
    let work = monitor.work_area();
    let scale = monitor.scale_factor();
    let threshold = (SNAP_THRESHOLD * scale).round() as i32;
    let mut x = position.x;
    let mut y = position.y;

    if config.behavior.edge_snap {
        let width = size.width as i32;
        let height = size.height as i32;
        let candidates_x = [
            work.position.x,
            work.position.x + (work.size.width as i32 - width) / 2,
            work.position.x + work.size.width as i32 - width,
        ];
        let candidates_y = [
            work.position.y,
            work.position.y + (work.size.height as i32 - height) / 2,
            work.position.y + work.size.height as i32 - height,
        ];
        if let Some(candidate) = nearest(x, &candidates_x, threshold) {
            x = candidate;
        }
        if let Some(candidate) = nearest(y, &candidates_y, threshold) {
            y = candidate;
        }
    }

    let width = size.width as i32;
    let min_x = work.position.x - width + 60;
    let max_x = work.position.x + work.size.width as i32 - 60;
    let min_y = work.position.y - 10;
    let max_y = work.position.y + work.size.height as i32 - 36;
    x = x.clamp(min_x, max_x.max(min_x));
    y = y.clamp(min_y, max_y.max(min_y));

    if position.x != x || position.y != y {
        let _ = window.set_position(PhysicalPosition::new(x, y));
        position.x = x;
        position.y = y;
    }

    let name = monitor_name(&monitor);
    if let Ok(mut memory) = state.window_memory.lock() {
        memory.last_monitor = name.clone();
        memory.positions.insert(
            name,
            SavedPosition {
                x: position.x,
                y: position.y,
            },
        );
        if memory.positions.len() > 24 {
            memory.positions.clear();
            memory.last_monitor.clear();
        }
        let snapshot = memory.clone();
        drop(memory);
        if let Ok(json) = serde_json::to_vec_pretty(&snapshot) {
            let _ = replace_file(&state.window_path, &json);
        }
    };
}

fn nearest(value: i32, candidates: &[i32], threshold: i32) -> Option<i32> {
    candidates
        .iter()
        .copied()
        .min_by_key(|candidate| (candidate - value).abs())
        .filter(|candidate| (candidate - value).abs() <= threshold)
}

pub fn update_visibility(app: &AppHandle) {
    let state = app.state::<AppState>();
    let config = state.config();
    let status = state.playback_status();
    let hidden_by_user = state.user_hidden.load(Ordering::SeqCst);
    let hidden_by_fullscreen =
        config.behavior.hide_on_fullscreen && state.fullscreen.load(Ordering::SeqCst);
    let peeking = state.peeking.load(Ordering::SeqCst);
    let paused_hidden = state.paused_hidden.load(Ordering::SeqCst);
    let settings_open = state.settings_open.load(Ordering::SeqCst);
    let active = !hidden_by_user
        && !hidden_by_fullscreen
        && (peeking
            || settings_open
            || status == "playing"
            || (status == "paused" && !paused_hidden));

    if active {
        show_widget(app);
    } else {
        hide_widget(app);
    }
}

pub fn show_widget(app: &AppHandle) {
    let state = app.state::<AppState>();
    let Some(window) = ensure_widget(app) else {
        return;
    };
    if !state.widget_loaded.load(Ordering::SeqCst) {
        return;
    }
    if state.widget_suspended.swap(false, Ordering::SeqCst) {
        resume_webview(&window);
    }
    let config = state.config();
    let _ = window.set_ignore_cursor_events(config.behavior.click_through);
    let _ = window.set_always_on_top(true);
    let _ = window.show();
    let _ = window.set_focusable(false);
    state.timers.cancel(crate::timers::TimerKind::HiddenDestroy);
    crate::lyrics::emit_position(app, &state.playback());
}

pub fn hide_widget(app: &AppHandle) {
    let state = app.state::<AppState>();
    let Some(window) = app.get_webview_window(WIDGET_LABEL) else {
        return;
    };
    let _ = window.hide();
    let _ = app.emit("widget-hidden", ());
    if state.widget_loaded.load(Ordering::SeqCst) {
        suspend_webview(app, &window);
    }
    state.timers.set(
        crate::timers::TimerKind::HiddenDestroy,
        Duration::from_secs(5),
    );
    crate::perf::trim_process_tree();
}

pub fn destroy_widget(app: &AppHandle) {
    let state = app.state::<AppState>();
    state.timers.cancel(crate::timers::TimerKind::HiddenDestroy);
    state.widget_loaded.store(false, Ordering::SeqCst);
    state.widget_suspended.store(false, Ordering::SeqCst);
    state.island_expanded.store(false, Ordering::SeqCst);
    if let Some(window) = app.get_webview_window(WIDGET_LABEL) {
        let _ = window.destroy();
    }
    let _ = app.emit("widget-hidden", ());
    crate::perf::trim_process_tree();
    let handle = app.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(1400));
        if handle.get_webview_window(WIDGET_LABEL).is_none() {
            crate::perf::trim_process_tree();
        }
    });
}

fn resume_webview(window: &WebviewWindow) {
    let _ = window.with_webview(|webview| {
        if let Ok(core) = webview.controller().cast::<ICoreWebView2_3>() {
            // SAFETY: the controller comes from this live webview; Resume only affects it.
            unsafe {
                let _ = core.Resume();
            }
        }
    });
}

fn suspend_webview(app: &AppHandle, window: &WebviewWindow) {
    let state = app.state::<AppState>();
    if state.widget_suspended.swap(true, Ordering::SeqCst) {
        return;
    }
    let handle = app.clone();
    let result = window.with_webview(move |webview| {
        let Ok(core) = webview.controller().cast::<ICoreWebView2_3>() else {
            suspend_failed(&handle);
            return;
        };
        let failure_handle = handle.clone();
        let handler = TrySuspendCompletedHandler::create(Box::new(move |_hr, success| {
            if !success {
                suspend_failed(&failure_handle);
            }
            Ok(())
        }));
        let suspended = unsafe {
            // SAFETY: the controller is live for this webview and the handler owns its callback.
            core.TrySuspend(&handler)
        };
        if suspended.is_err() {
            suspend_failed(&handle);
        }
    });
    if result.is_err() {
        suspend_failed(app);
    }
}

fn suspend_failed(app: &AppHandle) {
    let handle = app.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(80));
        let state = handle.state::<AppState>();
        state.widget_suspended.store(false, Ordering::SeqCst);
        destroy_widget(&handle);
    });
}

pub fn widget_ready(app: &AppHandle) {
    let state = app.state::<AppState>();
    state.widget_loaded.store(true, Ordering::SeqCst);
    update_visibility(app);
}

pub fn set_hover(app: &AppHandle, hovered: bool) {
    let state = app.state::<AppState>();
    let config = state.config();
    if config.mode != "island" {
        return;
    }
    state.island_expanded.store(hovered, Ordering::SeqCst);
    let Some(window) = app.get_webview_window(WIDGET_LABEL) else {
        return;
    };
    let (width, height) = widget_size(&config, hovered);
    resize_widget(&window, width, height);
}

pub fn apply_config(app: &AppHandle, config: &Config) {
    let Some(window) = app.get_webview_window(WIDGET_LABEL) else {
        return;
    };
    let state = app.state::<AppState>();
    if config.mode != "island" {
        state.island_expanded.store(false, Ordering::SeqCst);
    }
    let expanded = config.mode == "island" && state.island_expanded.load(Ordering::SeqCst);
    let (width, height) = widget_size(config, expanded);
    resize_widget(&window, width, height);
    apply_effects(&window, config);
    let _ = window.set_ignore_cursor_events(config.behavior.click_through);
    let _ = window.set_always_on_top(true);
    update_visibility(app);
}

fn resize_widget(window: &WebviewWindow, width: f64, height: f64) {
    let scale = window.scale_factor().unwrap_or(1.0);
    let Ok(current) = window.outer_size() else {
        return;
    };
    let target_width = (width * scale).round() as u32;
    let target_height = (height * scale).round() as u32;
    if current.width.abs_diff(target_width) <= 1 && current.height.abs_diff(target_height) <= 1 {
        return;
    }
    let position = window.outer_position().ok();
    let _ = window.set_size(LogicalSize::new(width, height));
    if current.width != target_width {
        if let Some(position) = position {
            let delta = (current.width as i64 - target_width as i64) / 2;
            let _ =
                window.set_position(PhysicalPosition::new(position.x + delta as i32, position.y));
        }
    }
}

fn apply_effects(window: &WebviewWindow, config: &Config) {
    match effects_for(config) {
        Some(effects) => {
            let _ = window.set_effects(effects);
        }
        None => {
            let _ = window.set_effects(None::<WindowEffectsConfig>);
        }
    }
}

pub fn set_fullscreen(app: &AppHandle, fullscreen: bool) {
    let state = app.state::<AppState>();
    if state.fullscreen.swap(fullscreen, Ordering::SeqCst) == fullscreen {
        return;
    }
    update_visibility(app);
}

pub fn toggle_user_hidden(app: &AppHandle) {
    let state = app.state::<AppState>();
    let visible = app
        .get_webview_window(WIDGET_LABEL)
        .and_then(|window| window.is_visible().ok())
        .unwrap_or(false);
    state.user_hidden.store(visible, Ordering::SeqCst);
    if !visible {
        state.paused_hidden.store(false, Ordering::SeqCst);
    }
    update_visibility(app);
}

pub fn peek(app: &AppHandle) {
    let state = app.state::<AppState>();
    let config = state.config();
    state.user_hidden.store(false, Ordering::SeqCst);
    state.paused_hidden.store(false, Ordering::SeqCst);
    state.peeking.store(true, Ordering::SeqCst);
    update_visibility(app);
    state.timers.set(
        crate::timers::TimerKind::PeekHide,
        Duration::from_secs(config.behavior.quick_peek_seconds.clamp(2, 30) as u64),
    );
}

pub fn open_settings(app: &AppHandle) {
    let state = app.state::<AppState>();
    state.user_hidden.store(false, Ordering::SeqCst);
    state.paused_hidden.store(false, Ordering::SeqCst);
    if let Some(window) = app.get_webview_window(SETTINGS_LABEL) {
        let _ = window.show();
        let _ = window.set_focus();
        update_visibility(app);
        return;
    }
    let data_dir = state.webview_dir.clone();
    let args = webview_args();
    match WebviewWindowBuilder::new(app, SETTINGS_LABEL, WebviewUrl::App("settings.html".into()))
        .title("Iceberg Settings")
        .inner_size(920.0, 700.0)
        .min_inner_size(760.0, 540.0)
        .resizable(true)
        .decorations(true)
        .transparent(false)
        .shadow(true)
        .center()
        .visible(true)
        .focused(true)
        .additional_browser_args(&args)
        .data_directory(data_dir)
        .build()
    {
        Ok(window) => {
            state.settings_open.store(true, Ordering::SeqCst);
            update_visibility(app);
            let handle = app.clone();
            window.on_window_event(move |event| {
                if matches!(
                    event,
                    WindowEvent::Destroyed | WindowEvent::CloseRequested { .. }
                ) {
                    handle
                        .state::<AppState>()
                        .settings_open
                        .store(false, Ordering::SeqCst);
                    update_visibility(&handle);
                }
            });
        }
        Err(error) => {
            eprintln!("iceberg: could not open settings: {error}");
        }
    }
}

pub fn close_settings(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(SETTINGS_LABEL) {
        let _ = window.destroy();
    }
    app.state::<AppState>()
        .settings_open
        .store(false, Ordering::SeqCst);
    update_visibility(app);
}

pub fn flush_position(app: &AppHandle) {
    save_position(app);
}
