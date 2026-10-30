use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;

use crate::config::Config;
use crate::state::AppState;
use crate::tray;
use crate::window;
use tauri::Manager;

pub fn apply_config(app: &AppHandle, config: &Config) {
    window::apply_config(app, config);
    crate::hotkeys::apply(app, config);
    sync_autostart(app, config.behavior.start_with_windows);
    tray::refresh(app);
    let state = app.state::<AppState>();
    crate::lyrics::emit_config(app, config, state.effective_offset());
    let sender = state.smtc_poke.lock().ok().and_then(|slot| slot.clone());
    if let Some(sender) = sender {
        let _ = sender.send(());
    }
}

pub fn cycle_mode(app: &AppHandle) {
    let state = app.state::<AppState>();
    let mut config = state.config();
    config.mode = match config.mode.as_str() {
        "island" => "card".to_string(),
        "card" => "lyrics".to_string(),
        _ => "island".to_string(),
    };
    let config = state.update_config(config);
    apply_config(app, &config);
}

pub fn toggle_click_through(app: &AppHandle) {
    let state = app.state::<AppState>();
    let mut config = state.config();
    config.behavior.click_through = !config.behavior.click_through;
    let config = state.update_config(config);
    apply_config(app, &config);
}

fn sync_autostart(app: &AppHandle, enabled: bool) {
    let manager = app.autolaunch();
    match manager.is_enabled() {
        Ok(current) if current == enabled => {}
        Ok(_) => {
            let result = if enabled {
                manager.enable()
            } else {
                manager.disable()
            };
            if let Err(error) = result {
                eprintln!("iceberg: autostart update failed: {error}");
            }
        }
        Err(error) => {
            eprintln!("iceberg: autostart state unavailable: {error}");
        }
    }
}
