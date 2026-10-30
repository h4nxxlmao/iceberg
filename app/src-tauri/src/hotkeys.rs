use std::sync::{Mutex, OnceLock};

use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::config::Config;

#[derive(Clone, Copy)]
enum Action {
    Toggle,
    ClickThrough,
    OffsetUp,
    OffsetDown,
    Mode,
    Peek,
    Settings,
}

pub fn apply(app: &AppHandle, config: &Config) {
    let signature = format!(
        "{}|{}|{}|{}|{}|{}|{}",
        config.hotkeys.toggle,
        config.hotkeys.click_through,
        config.hotkeys.offset_up,
        config.hotkeys.offset_down,
        config.hotkeys.mode,
        config.hotkeys.peek,
        config.hotkeys.settings
    );

    static REGISTERED: OnceLock<Mutex<String>> = OnceLock::new();
    let registered = REGISTERED.get_or_init(|| Mutex::new(String::new()));
    if let Ok(current) = registered.lock() {
        if *current == signature {
            return;
        }
    }

    let manager = app.global_shortcut();
    let _ = manager.unregister_all();

    for (key, action) in [
        (config.hotkeys.toggle.as_str(), Action::Toggle),
        (config.hotkeys.click_through.as_str(), Action::ClickThrough),
        (config.hotkeys.offset_up.as_str(), Action::OffsetUp),
        (config.hotkeys.offset_down.as_str(), Action::OffsetDown),
        (config.hotkeys.mode.as_str(), Action::Mode),
        (config.hotkeys.peek.as_str(), Action::Peek),
        (config.hotkeys.settings.as_str(), Action::Settings),
    ] {
        if key.trim().is_empty() {
            continue;
        }
        let Ok(shortcut) = key.parse::<Shortcut>() else {
            eprintln!("iceberg: invalid hotkey '{key}'");
            continue;
        };
        if let Err(error) = manager.on_shortcut(shortcut, move |app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                handle(app, action);
            }
        }) {
            eprintln!("iceberg: could not register hotkey '{key}': {error}");
        }
    }

    if let Ok(mut current) = registered.lock() {
        *current = signature;
    }
}

fn handle(app: &AppHandle, action: Action) {
    let state = app.state::<crate::state::AppState>();
    match action {
        Action::Toggle => crate::window::toggle_user_hidden(app),
        Action::ClickThrough => crate::apply::toggle_click_through(app),
        Action::OffsetUp => {
            crate::lyrics::set_offset(&state, app, 100);
        }
        Action::OffsetDown => {
            crate::lyrics::set_offset(&state, app, -100);
        }
        Action::Mode => crate::apply::cycle_mode(app),
        Action::Peek => crate::window::peek(app),
        Action::Settings => crate::window::open_settings(app),
    }
}
