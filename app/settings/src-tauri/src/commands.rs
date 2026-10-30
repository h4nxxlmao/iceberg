use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use base64::Engine as _;
use iceberg_core::config::{
    Background, Behavior, Config, Hotkeys, Layout, LyricsSettings, Typography,
};
use iceberg_core::fonts::{self, FontFamilyInfo};
use iceberg_core::ipc::{Client, IpcMessage, IpcStatus};
use iceberg_core::paths::{assets_dir, Paths};
use iceberg_core::presets::{builtin_presets, Preset};
use serde::{Deserialize, Serialize};
use tauri::State;

pub struct SettingsState {
    paths: Paths,
    section: String,
    client: Mutex<Option<Client>>,
}

impl SettingsState {
    pub fn new(paths: Paths, section: String) -> Self {
        Self {
            paths,
            section,
            client: Mutex::new(None),
        }
    }

    pub fn close(&self) {
        if let Ok(mut guard) = self.client.lock() {
            if let Some(client) = guard.as_mut() {
                let _ = client.request(&IpcMessage::CloseSettings);
            }
            *guard = None;
        }
    }

    fn ensure_client(&self) -> Result<(), String> {
        let mut guard = self
            .client
            .lock()
            .map_err(|_| "The widget client is unavailable".to_string())?;
        if guard.is_some() {
            return Ok(());
        }
        if let Some(client) = try_connect() {
            *guard = Some(client);
            return Ok(());
        }
        spawn_widget()?;
        for _ in 0..40 {
            std::thread::sleep(Duration::from_millis(75));
            if let Some(client) = try_connect() {
                *guard = Some(client);
                return Ok(());
            }
        }
        Err("The Iceberg widget did not open its IPC pipe".to_string())
    }

    fn request(&self, message: &IpcMessage) -> Result<IpcMessage, String> {
        self.ensure_client()?;
        let mut guard = self
            .client
            .lock()
            .map_err(|_| "The widget client is unavailable".to_string())?;
        let Some(client) = guard.as_mut() else {
            return Err("The widget client is unavailable".to_string());
        };
        match client.request(message) {
            Ok(response) => Ok(response),
            Err(_) => {
                *guard = None;
                drop(guard);
                self.ensure_client()?;
                let mut guard = self
                    .client
                    .lock()
                    .map_err(|_| "The widget client is unavailable".to_string())?;
                let Some(client) = guard.as_mut() else {
                    return Err("The widget client is unavailable".to_string());
                };
                client
                    .request(message)
                    .map_err(|error| format!("Widget IPC failed: {error}"))
            }
        }
    }

    fn get_config(&self) -> Result<Config, String> {
        match self.request(&IpcMessage::GetConfig)? {
            IpcMessage::ConfigChanged { config } => Ok(config),
            IpcMessage::Error { message } => Err(message),
            other => Err(format!("Unexpected config response: {other:?}")),
        }
    }

    fn get_status(&self) -> Result<IpcStatus, String> {
        match self.request(&IpcMessage::GetStatus)? {
            IpcMessage::Status { status } => Ok(status),
            IpcMessage::Error { message } => Err(message),
            other => Err(format!("Unexpected status response: {other:?}")),
        }
    }

    fn commit_config(&self, mut config: Config) -> Result<Config, String> {
        config.validate();
        for value in [
            &config.hotkeys.toggle,
            &config.hotkeys.click_through,
            &config.hotkeys.offset_up,
            &config.hotkeys.offset_down,
            &config.hotkeys.mode,
            &config.hotkeys.peek,
            &config.hotkeys.settings,
        ] {
            if !value.trim().is_empty() {
                iceberg_core::hotkeys::parse(value)
                    .map_err(|error| format!("Hotkey '{value}': {error}"))?;
            }
        }
        match self.request(&IpcMessage::SetConfig { config })? {
            IpcMessage::ConfigChanged { config } => Ok(config),
            IpcMessage::Error { message } => Err(message),
            other => Err(format!("Unexpected config response: {other:?}")),
        }
    }
}

fn try_connect() -> Option<Client> {
    let mut client = Client::connect().ok()?;
    match client.request(&IpcMessage::Ping) {
        Ok(IpcMessage::Pong) => Some(client),
        _ => None,
    }
}

fn spawn_widget() -> Result<(), String> {
    let exe = std::env::current_exe()
        .map_err(|error| error.to_string())?
        .parent()
        .map(|path| path.join("iceberg.exe"))
        .ok_or_else(|| "Could not locate the widget executable".to_string())?;
    if !exe.is_file() {
        return Err(format!(
            "The Iceberg widget was not found next to Settings (expected {}). Build it with `cargo build -p iceberg-widget` first.",
            exe.display()
        ));
    }
    std::process::Command::new(&exe)
        .spawn()
        .map_err(|error| format!("Could not start the Iceberg widget: {error}"))?;
    Ok(())
}

#[derive(Serialize)]
pub struct Bootstrap {
    config: Config,
    status: IpcStatus,
    presets: Vec<Preset>,
    fonts: Vec<FontFamilyInfo>,
    section: String,
}

#[derive(Serialize)]
pub struct SyncState {
    config: Config,
    status: IpcStatus,
}

#[derive(Serialize)]
pub struct FontPreview {
    family: String,
    data: String,
}

#[derive(Deserialize)]
struct FontPreviewEntry {
    family: String,
    file: String,
}

#[derive(Serialize)]
pub struct FontUpload {
    family: String,
    fonts: Vec<FontFamilyInfo>,
}

#[derive(Serialize)]
pub struct ImageData {
    data: String,
}

#[tauri::command]
pub fn bootstrap(state: State<'_, SettingsState>) -> Result<Bootstrap, String> {
    state.ensure_client()?;
    let _ = state.request(&IpcMessage::OpenSettings)?;
    std::thread::sleep(Duration::from_millis(80));
    let config = state.get_config()?;
    let status = state.get_status()?;
    Ok(Bootstrap {
        config,
        status,
        presets: builtin_presets(),
        fonts: fonts::catalog(&state.paths.fonts),
        section: state.section.clone(),
    })
}

#[tauri::command]
pub fn refresh(state: State<'_, SettingsState>) -> Result<SyncState, String> {
    Ok(SyncState {
        config: state.get_config()?,
        status: state.get_status()?,
    })
}

#[tauri::command]
pub fn save_config(state: State<'_, SettingsState>, config: Config) -> Result<Config, String> {
    state.commit_config(config)
}

#[tauri::command]
pub fn apply_preset(state: State<'_, SettingsState>, id: String) -> Result<Config, String> {
    let mut config = state.get_config()?;
    let preset = builtin_presets()
        .into_iter()
        .find(|preset| preset.id == id)
        .ok_or_else(|| format!("Unknown preset '{id}'"))?;
    preset.apply_to(&mut config);
    state.commit_config(config)
}

#[tauri::command]
pub fn reset_section(state: State<'_, SettingsState>, section: String) -> Result<Config, String> {
    let mut config = state.get_config()?;
    match section.as_str() {
        "appearance" => {
            config.background = Background::default();
        }
        "typography" => {
            config.typography = Typography::default();
        }
        "layout" => {
            config.layout = Layout::default();
        }
        "playback" => {
            config.behavior = Behavior::default();
        }
        "lyrics" => {
            config.lyrics = LyricsSettings::default();
            config.per_song.clear();
        }
        "hotkeys" => {
            config.hotkeys = Hotkeys::default();
        }
        "all" => {
            config = Config::default();
        }
        other => return Err(format!("Unknown section '{other}'")),
    }
    config.active_preset = None;
    state.commit_config(config)
}

#[tauri::command]
pub fn list_fonts(state: State<'_, SettingsState>) -> Vec<FontFamilyInfo> {
    fonts::catalog(&state.paths.fonts)
}

#[tauri::command]
pub fn font_previews(text: String) -> Result<Vec<FontPreview>, String> {
    let exe = std::env::current_exe()
        .map_err(|error| error.to_string())?
        .parent()
        .map(|path| path.join("iceberg.exe"))
        .ok_or_else(|| "Could not locate the widget executable".to_string())?;
    if !exe.is_file() {
        return Err("The widget executable is missing".to_string());
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let directory = std::env::temp_dir().join(format!(
        "iceberg-font-previews-{}-{stamp}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let text: String = text
        .chars()
        .take(80)
        .collect::<String>()
        .replace(['\r', '\n'], " ");
    let output = std::process::Command::new(&exe)
        .arg(format!("--font-previews={}", directory.display()))
        .arg(format!("--preview-text={text}"))
        .output()
        .map_err(|error| format!("Could not render font specimens: {error}"))?;
    if !output.status.success() {
        let _ = std::fs::remove_dir_all(&directory);
        return Err(format!(
            "Font specimen renderer failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let result = (|| {
        let manifest = std::fs::read(directory.join("previews.json"))
            .map_err(|error| format!("Missing font preview manifest: {error}"))?;
        let entries: Vec<FontPreviewEntry> =
            serde_json::from_slice(&manifest).map_err(|error| error.to_string())?;
        let mut previews = Vec::with_capacity(entries.len());
        for entry in entries {
            let bytes = std::fs::read(directory.join(&entry.file))
                .map_err(|error| format!("Missing font preview: {error}"))?;
            previews.push(FontPreview {
                family: entry.family,
                data: base64::engine::general_purpose::STANDARD.encode(bytes),
            });
        }
        Ok(previews)
    })();
    let _ = std::fs::remove_dir_all(&directory);
    result
}

#[tauri::command]
pub fn upload_font(state: State<'_, SettingsState>) -> Result<FontUpload, String> {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("Fonts", &["ttf", "otf"])
        .set_title("Add a font to Iceberg")
        .pick_file()
    else {
        return Err("No font selected".to_string());
    };
    let info = fonts::load_font(&path).map_err(|error| format!("Invalid font: {error}"))?;
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("ttf")
        .to_ascii_lowercase();
    if extension != "ttf" && extension != "otf" {
        return Err("Only .ttf and .otf files are supported".to_string());
    }
    let stem: String = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("font")
        .chars()
        .filter(|character| {
            character.is_ascii_alphanumeric() || *character == '-' || *character == '_'
        })
        .take(48)
        .collect();
    let stem = if stem.is_empty() {
        "font".to_string()
    } else {
        stem
    };
    let target = unique_font_path(&state.paths.fonts, &stem, &extension);
    std::fs::copy(&path, &target).map_err(|error| format!("Could not import the font: {error}"))?;
    let _ = state.request(&IpcMessage::FontsChanged);
    Ok(FontUpload {
        family: info.family,
        fonts: fonts::catalog(&state.paths.fonts),
    })
}

fn unique_font_path(directory: &Path, stem: &str, extension: &str) -> PathBuf {
    let mut index = 0;
    loop {
        let name = if index == 0 {
            format!("{stem}.{extension}")
        } else {
            format!("{stem}-{index}.{extension}")
        };
        let path = directory.join(name);
        if !path.exists() {
            return path;
        }
        index += 1;
    }
}

fn image_for(directory: &str, name: &str) -> Result<ImageData, String> {
    let path = assets_dir().join(directory).join(format!("{name}.png"));
    let bytes =
        std::fs::read(&path).map_err(|_| format!("Missing native preview {}", path.display()))?;
    Ok(ImageData {
        data: base64::engine::general_purpose::STANDARD.encode(bytes),
    })
}

#[tauri::command]
pub fn preset_image(id: String) -> Result<ImageData, String> {
    if !builtin_presets().iter().any(|preset| preset.id == id) {
        return Err(format!("Unknown preset '{id}'"));
    }
    image_for("presets", &id)
}

#[tauri::command]
pub fn surface_image(kind: String) -> Result<ImageData, String> {
    if !matches!(
        kind.as_str(),
        "solid" | "transparent" | "acrylic" | "ice" | "liquid"
    ) {
        return Err(format!("Unknown surface '{kind}'"));
    }
    image_for("surfaces", &kind)
}

#[tauri::command]
pub fn show_widget(state: State<'_, SettingsState>) -> Result<(), String> {
    match state.request(&IpcMessage::ShowWidget)? {
        IpcMessage::Pong => Ok(()),
        IpcMessage::Error { message } => Err(message),
        other => Err(format!("Unexpected widget response: {other:?}")),
    }
}
