#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;

use iceberg_core::paths::Paths;
use tauri::Manager;

fn main() {
    let Some(_settings_instance) = iceberg_core::single_instance::acquire("iceberg-settings")
    else {
        return;
    };
    let section = std::env::args()
        .find_map(|value| value.strip_prefix("--section=").map(str::to_string))
        .unwrap_or_else(|| "appearance".to_string());
    let paths = Paths::resolve();
    let _ = paths.ensure();
    let state = commands::SettingsState::new(paths, section);

    tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::bootstrap,
            commands::refresh,
            commands::status,
            commands::save_config,
            commands::apply_preset,
            commands::reset_section,
            commands::list_fonts,
            commands::font_previews,
            commands::upload_font,
            commands::preset_image,
            commands::surface_image,
            commands::show_widget,
            commands::retry_lyrics,
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                let state = window.state::<commands::SettingsState>();
                state.close();
                window.app_handle().exit(0);
            }
        })
        .run(tauri::generate_context!())
        .expect("Iceberg Settings failed to start");
}
