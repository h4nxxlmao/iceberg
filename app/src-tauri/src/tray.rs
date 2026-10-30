use std::sync::{Mutex, OnceLock};

use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};

use crate::config::{self, Config};
use crate::state::AppState;
use crate::window;

const TRAY_ID: &str = "iceberg-tray";

fn menu_signature(config: &Config) -> String {
    format!(
        "{}|{}|{}|{}",
        config.mode,
        config.theme,
        config.behavior.click_through,
        config.behavior.start_with_windows
    )
}

fn last_signature() -> &'static Mutex<String> {
    static LAST: OnceLock<Mutex<String>> = OnceLock::new();
    LAST.get_or_init(|| Mutex::new(String::new()))
}

pub fn init(app: &AppHandle) -> tauri::Result<()> {
    let state = app.state::<AppState>();
    let config = state.config();
    let menu = build_menu(app, &config)?;
    let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/tray.png"))?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .tooltip("Iceberg")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| handle_menu(app, event.id().as_ref()))
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                window::toggle_user_hidden(tray.app_handle());
            }
        })
        .build(app)?;

    if let Ok(mut last) = last_signature().lock() {
        *last = menu_signature(&config);
    }
    set_playing(app, false);
    Ok(())
}

pub fn refresh(app: &AppHandle) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return;
    };
    let state = app.state::<AppState>();
    let config = state.config();
    let signature = menu_signature(&config);
    if let Ok(last) = last_signature().lock() {
        if *last == signature {
            return;
        }
    }
    if let Ok(menu) = build_menu(app, &config) {
        let _ = tray.set_menu(Some(menu));
        if let Ok(mut last) = last_signature().lock() {
            *last = signature;
        }
    }
}

fn build_menu(app: &AppHandle, config: &Config) -> tauri::Result<Menu<Wry>> {
    let toggle = MenuItem::with_id(app, "toggle", "Show / hide widget", true, None::<&str>)?;

    let island = CheckMenuItem::with_id(
        app,
        "mode:island",
        "Island",
        true,
        config.mode == "island",
        None::<&str>,
    )?;
    let card = CheckMenuItem::with_id(
        app,
        "mode:card",
        "Card",
        true,
        config.mode == "card",
        None::<&str>,
    )?;
    let lyrics_mode = CheckMenuItem::with_id(
        app,
        "mode:lyrics",
        "Lyrics only",
        true,
        config.mode == "lyrics",
        None::<&str>,
    )?;
    let mode = Submenu::with_items(app, "Mode", true, &[&island, &card, &lyrics_mode])?;

    let lock = CheckMenuItem::with_id(
        app,
        "lock",
        "Lock position (click-through)",
        true,
        config.behavior.click_through,
        None::<&str>,
    )?;

    let offset_down = MenuItem::with_id(app, "offset_down", "Lyrics − 0.5 s", true, None::<&str>)?;
    let offset_up = MenuItem::with_id(app, "offset_up", "Lyrics + 0.5 s", true, None::<&str>)?;
    let offset_reset =
        MenuItem::with_id(app, "offset_reset", "Reset song offset", true, None::<&str>)?;
    let offsets = Submenu::with_items(
        app,
        "Lyrics offset",
        true,
        &[&offset_down, &offset_up, &offset_reset],
    )?;

    let dark = CheckMenuItem::with_id(
        app,
        "theme:dark-glass",
        "Glacier",
        true,
        config.theme == "dark-glass",
        None::<&str>,
    )?;
    let light = CheckMenuItem::with_id(
        app,
        "theme:light",
        "Paper",
        true,
        config.theme == "light",
        None::<&str>,
    )?;
    let minimal = CheckMenuItem::with_id(
        app,
        "theme:minimal",
        "Graphite",
        true,
        config.theme == "minimal",
        None::<&str>,
    )?;
    let neon = CheckMenuItem::with_id(
        app,
        "theme:neon",
        "Ember",
        true,
        config.theme == "neon",
        None::<&str>,
    )?;
    let mono = CheckMenuItem::with_id(
        app,
        "theme:mono",
        "Frost",
        true,
        config.theme == "mono",
        None::<&str>,
    )?;
    let album = CheckMenuItem::with_id(
        app,
        "theme:album-color",
        "Cover",
        true,
        config.theme == "album-color",
        None::<&str>,
    )?;
    let themes = Submenu::with_items(
        app,
        "Theme",
        true,
        &[&dark, &light, &minimal, &neon, &mono, &album],
    )?;

    let autostart = CheckMenuItem::with_id(
        app,
        "autostart",
        "Start with Windows",
        true,
        config.behavior.start_with_windows,
        None::<&str>,
    )?;
    let settings = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Iceberg", true, None::<&str>)?;

    Menu::with_items(
        app,
        &[
            &toggle, &mode, &lock, &offsets, &themes, &autostart, &separator, &settings, &quit,
        ],
    )
}

fn handle_menu(app: &AppHandle, id: &str) {
    let state = app.state::<AppState>();
    match id {
        "toggle" => window::toggle_user_hidden(app),
        "lock" => crate::apply::toggle_click_through(app),
        "offset_down" => {
            crate::lyrics::set_offset(&state, app, -500);
        }
        "offset_up" => {
            crate::lyrics::set_offset(&state, app, 500);
        }
        "offset_reset" => {
            crate::lyrics::reset_offset(&state, app);
        }
        "autostart" => {
            let mut config = state.config();
            config.behavior.start_with_windows = !config.behavior.start_with_windows;
            let config = state.update_config(config);
            crate::apply::apply_config(app, &config);
        }
        "settings" => window::open_settings(app),
        "quit" => quit(app),
        other if other.starts_with("mode:") => {
            let mode = other.trim_start_matches("mode:").to_string();
            if !matches!(mode.as_str(), "island" | "card" | "lyrics") {
                return;
            }
            let mut config = state.config();
            config.mode = mode;
            let config = state.update_config(config);
            crate::apply::apply_config(app, &config);
        }
        other if other.starts_with("theme:") => {
            let theme = other.trim_start_matches("theme:");
            let mut config = state.config();
            if config::apply_theme(&mut config, theme) {
                let config = state.update_config(config);
                crate::apply::apply_config(app, &config);
            }
        }
        _ => {}
    }
}

fn quit(app: &AppHandle) {
    let state = app.state::<AppState>();
    state
        .quitting
        .store(true, std::sync::atomic::Ordering::SeqCst);
    window::flush_position(app);
    state.store.flush(&state.config());
    state.timers.shutdown();
    app.exit(0);
}

pub fn set_playing(app: &AppHandle, playing: bool) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return;
    };
    let bytes: &[u8] = if playing {
        include_bytes!("../icons/tray.png")
    } else {
        include_bytes!("../icons/tray-dim.png")
    };
    if let Ok(icon) = tauri::image::Image::from_bytes(bytes) {
        let _ = tray.set_icon(Some(icon));
    }
    let state = app.state::<AppState>();
    let tooltip = if playing {
        state
            .current_track()
            .map(|track| format!("Iceberg: {}", track.title))
            .unwrap_or_else(|| "Iceberg".to_string())
    } else {
        "Iceberg".to_string()
    };
    let _ = tray.set_tooltip(Some(tooltip));
}
