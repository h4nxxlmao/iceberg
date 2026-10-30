use std::fs;

use serde::Serialize;
use tauri::{AppHandle, Manager};
use windows::Win32::Foundation::{GlobalFree, HANDLE};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use windows::Win32::System::Ole::CF_UNICODETEXT;

use crate::config::{self, Appearance, Config};
use crate::lyrics::{self, SearchResult};
use crate::perf::Diagnostics;
use crate::state::{AppState, Snapshot};
use crate::util::{self, read_limited};
use crate::{apply, cache, window};

#[tauri::command]
pub fn get_snapshot(app: AppHandle) -> Snapshot {
    let state = app.state::<AppState>();
    state.cache_bytes.store(
        cache::cache_size(&state.cache_dir),
        std::sync::atomic::Ordering::Relaxed,
    );
    state.snapshot()
}

#[tauri::command]
pub fn set_config(app: AppHandle, mut config: Config) -> Result<Config, String> {
    let state = app.state::<AppState>();
    config.per_song_offsets = state.config().per_song_offsets;
    let config = state.update_config(config);
    apply::apply_config(&app, &config);
    Ok(config)
}

#[tauri::command]
pub fn reset_config(app: AppHandle) -> Config {
    let state = app.state::<AppState>();
    let config = state.update_config(Config::default());
    apply::apply_config(&app, &config);
    config
}

#[tauri::command]
pub fn apply_theme(app: AppHandle, theme: String) -> Result<Config, String> {
    let state = app.state::<AppState>();
    let mut config = state.config();
    if !config::apply_theme(&mut config, &theme) {
        return Err("Unknown theme".to_string());
    }
    let config = state.update_config(config);
    apply::apply_config(&app, &config);
    Ok(config)
}

#[tauri::command]
pub fn set_mode(app: AppHandle, mode: String) -> Result<(), String> {
    if !matches!(mode.as_str(), "island" | "card" | "lyrics") {
        return Err("Unknown mode".to_string());
    }
    let state = app.state::<AppState>();
    let mut config = state.config();
    config.mode = mode;
    let config = state.update_config(config);
    apply::apply_config(&app, &config);
    Ok(())
}

#[tauri::command]
pub fn widget_ready(app: AppHandle) {
    window::widget_ready(&app);
}

#[tauri::command]
pub fn set_widget_hover(app: AppHandle, hovered: bool) {
    window::set_hover(&app, hovered);
}

#[tauri::command]
pub fn expand_for_track(app: AppHandle) {
    window::set_hover(&app, true);
}

#[tauri::command]
pub fn adjust_offset(app: AppHandle, delta: i64) -> i64 {
    let state = app.state::<AppState>();
    lyrics::set_offset(&state, &app, delta.clamp(-1000, 1000))
}

#[tauri::command]
pub fn reset_song_offset(app: AppHandle) -> i64 {
    let state = app.state::<AppState>();
    lyrics::reset_offset(&state, &app)
}

#[tauri::command(async)]
pub fn search_lyrics(app: AppHandle, query: String) -> Result<Vec<SearchResult>, String> {
    let results = lyrics::search_free(&query)?;
    app.state::<AppState>().set_search_results(results.clone());
    Ok(results)
}

#[tauri::command(async)]
pub fn apply_lyrics_choice(app: AppHandle, id: u64) -> Result<(), String> {
    let state = app.state::<AppState>();
    lyrics::apply_search_choice(&state, &app, id)
}

#[tauri::command]
pub fn reapply_lyrics(app: AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let track = state.current_track().ok_or("Nothing is playing")?;
    lyrics::start_fetch(app.clone(), track, true);
    Ok(())
}

#[tauri::command(async)]
pub fn import_lrc(app: AppHandle) -> Result<String, String> {
    let path = rfd::FileDialog::new()
        .add_filter("Lyrics", &["lrc", "txt"])
        .set_title("Import lyrics for the current song")
        .pick_file()
        .ok_or("No file selected")?;
    let text = read_limited(&path, 1_000_000).map_err(|error| error.to_string())?;
    let state = app.state::<AppState>();
    let result = lyrics::apply_manual(&state, &app, &text)?;
    if result.kind == lyrics::LyricsKind::Synced {
        Ok(format!("Imported {} timed lines", result.lines.len()))
    } else {
        Ok("Imported plain lyrics".to_string())
    }
}

#[tauri::command]
pub fn get_diagnostics(app: AppHandle) -> Diagnostics {
    crate::perf::diagnostics(&app)
}

#[tauri::command]
pub fn clear_cache(app: AppHandle) -> u64 {
    let state = app.state::<AppState>();
    let freed = cache::clear(&state.cache_dir);
    state.cache_bytes.store(
        cache::cache_size(&state.cache_dir),
        std::sync::atomic::Ordering::Relaxed,
    );
    freed
}

#[tauri::command]
pub fn open_config_folder(app: AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let parent = state
        .config_path
        .parent()
        .ok_or("Config folder is unavailable")?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    util::open_folder(parent);
    Ok(())
}

#[tauri::command]
pub fn open_settings(app: AppHandle) {
    window::open_settings(&app);
}

#[tauri::command]
pub fn close_settings(app: AppHandle) {
    window::close_settings(&app);
}

#[derive(Serialize)]
struct ThemeFile<'a> {
    name: &'a str,
    appearance: &'a Appearance,
}

#[tauri::command(async)]
pub fn export_theme(app: AppHandle) -> Result<String, String> {
    let state = app.state::<AppState>();
    let config = state.config();
    let path = rfd::FileDialog::new()
        .add_filter("Iceberg theme", &["json"])
        .set_file_name(format!("iceberg-{}.json", config.theme))
        .set_title("Export Iceberg theme")
        .save_file()
        .ok_or("No file selected")?;
    let payload = ThemeFile {
        name: &config.theme,
        appearance: &config.appearance,
    };
    let json = serde_json::to_vec_pretty(&payload).map_err(|error| error.to_string())?;
    fs::write(&path, json).map_err(|error| error.to_string())?;
    Ok(format!("Theme exported to {}", path.display()))
}

#[tauri::command(async)]
pub fn import_theme(app: AppHandle) -> Result<String, String> {
    let path = rfd::FileDialog::new()
        .add_filter("Iceberg theme", &["json"])
        .set_title("Import Iceberg theme")
        .pick_file()
        .ok_or("No file selected")?;
    let text = read_limited(&path, 500_000).map_err(|error| error.to_string())?;
    let value: serde_json::Value =
        serde_json::from_str(&text).map_err(|error| format!("Invalid theme: {error}"))?;
    let name = value
        .get("name")
        .and_then(|value| value.as_str())
        .unwrap_or("Imported theme")
        .chars()
        .take(60)
        .collect::<String>();
    let appearance_value = value.get("appearance").cloned().unwrap_or(value);
    let appearance: Appearance = serde_json::from_value(appearance_value)
        .map_err(|error| format!("Invalid appearance: {error}"))?;

    let mut probe = Config {
        appearance,
        ..Config::default()
    };
    probe.validate();

    let state = app.state::<AppState>();
    let mut config = state.config();
    config.appearance = probe.appearance;
    config.theme = "custom".to_string();
    let config = state.update_config(config);
    apply::apply_config(&app, &config);
    Ok(name)
}

#[tauri::command]
pub fn copy_text(text: String) -> Result<(), String> {
    if text.len() > 20_000 {
        return Err("Text is too long".to_string());
    }
    let utf16: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let bytes = utf16.len() * std::mem::size_of::<u16>();
    // SAFETY: the clipboard is opened and closed on this thread; the HGLOBAL is owned by the
    // clipboard after SetClipboardData succeeds and is freed explicitly otherwise.
    unsafe {
        OpenClipboard(None).map_err(|error| error.to_string())?;
        if EmptyClipboard().is_err() {
            let _ = CloseClipboard();
            return Err("Could not clear the clipboard".to_string());
        }
        let memory = match GlobalAlloc(GMEM_MOVEABLE, bytes) {
            Ok(memory) => memory,
            Err(error) => {
                let _ = CloseClipboard();
                return Err(error.to_string());
            }
        };
        let pointer = GlobalLock(memory);
        if pointer.is_null() {
            let _ = GlobalFree(Some(memory));
            let _ = CloseClipboard();
            return Err("Could not lock clipboard memory".to_string());
        }
        std::ptr::copy_nonoverlapping(utf16.as_ptr() as *const u8, pointer as *mut u8, bytes);
        let _ = GlobalUnlock(memory);
        if SetClipboardData(CF_UNICODETEXT.0 as u32, Some(HANDLE(memory.0))).is_err() {
            let _ = GlobalFree(Some(memory));
            let _ = CloseClipboard();
            return Err("Could not write to the clipboard".to_string());
        }
        let _ = CloseClipboard();
    }
    Ok(())
}
