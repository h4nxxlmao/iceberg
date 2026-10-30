use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use crate::util::{clamp_f64, clamp_i64, replace_file, sanitize_apps, sanitize_hex};

pub const CONFIG_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub version: u32,
    pub theme: String,
    pub mode: String,
    pub appearance: Appearance,
    pub lyrics: LyricsSettings,
    pub behavior: Behavior,
    pub hotkeys: Hotkeys,
    pub per_song_offsets: HashMap<String, i64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Appearance {
    pub font_family: String,
    pub font_size: u32,
    pub font_weight: u32,
    pub text_color: String,
    pub accent_color: String,
    pub background_color: String,
    pub opacity: f64,
    pub radius: u32,
    pub effect: String,
    pub accent_from_art: bool,
    pub lyric_animation: String,
    pub lyric_lines: u32,
    pub align: String,
    pub show_cover: bool,
    pub show_progress: bool,
    pub show_title: bool,
    pub show_artist: bool,
    pub border: bool,
    pub shadow: bool,
    pub reduce_motion: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct LyricsSettings {
    pub offset_ms: i64,
    pub unsynced: String,
    pub cjk_fallback: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Behavior {
    pub start_with_windows: bool,
    pub idle_destroy_minutes: u32,
    pub auto_hide_paused_seconds: u32,
    pub hide_on_fullscreen: bool,
    pub click_through: bool,
    pub edge_snap: bool,
    pub preferred_apps: Vec<String>,
    pub ignored_apps: Vec<String>,
    pub battery_saver: bool,
    pub fade_after_seconds: u32,
    pub quick_peek_seconds: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Hotkeys {
    pub toggle: String,
    pub click_through: String,
    pub offset_up: String,
    pub offset_down: String,
    pub mode: String,
    pub peek: String,
    pub settings: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            theme: "dark-glass".to_string(),
            mode: "card".to_string(),
            appearance: Appearance::default(),
            lyrics: LyricsSettings::default(),
            behavior: Behavior::default(),
            hotkeys: Hotkeys::default(),
            per_song_offsets: HashMap::new(),
        }
    }
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            font_family: "Segoe UI Variable Display".to_string(),
            font_size: 14,
            font_weight: 400,
            text_color: "#ECEAE1".to_string(),
            accent_color: "#F07A4F".to_string(),
            background_color: "#0C1012".to_string(),
            opacity: 0.92,
            radius: 6,
            effect: "none".to_string(),
            accent_from_art: false,
            lyric_animation: "slide".to_string(),
            lyric_lines: 4,
            align: "left".to_string(),
            show_cover: true,
            show_progress: true,
            show_title: true,
            show_artist: true,
            border: true,
            shadow: false,
            reduce_motion: false,
        }
    }
}

impl Default for LyricsSettings {
    fn default() -> Self {
        Self {
            offset_ms: 0,
            unsynced: "static".to_string(),
            cjk_fallback: true,
        }
    }
}

impl Default for Behavior {
    fn default() -> Self {
        Self {
            start_with_windows: false,
            idle_destroy_minutes: 5,
            auto_hide_paused_seconds: 30,
            hide_on_fullscreen: true,
            click_through: false,
            edge_snap: true,
            preferred_apps: Vec::new(),
            ignored_apps: Vec::new(),
            battery_saver: false,
            fade_after_seconds: 0,
            quick_peek_seconds: 5,
        }
    }
}

impl Default for Hotkeys {
    fn default() -> Self {
        Self {
            toggle: "Ctrl+Alt+KeyI".to_string(),
            click_through: "Ctrl+Alt+KeyL".to_string(),
            offset_up: "Ctrl+Alt+ArrowRight".to_string(),
            offset_down: "Ctrl+Alt+ArrowLeft".to_string(),
            mode: "Ctrl+Alt+KeyM".to_string(),
            peek: "Ctrl+Alt+KeyP".to_string(),
            settings: "Ctrl+Alt+Comma".to_string(),
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> Self {
        let Ok(text) = fs::read_to_string(path) else {
            return Self::default();
        };
        let text = text.trim_start_matches('\u{feff}');
        match serde_json::from_str::<Config>(text) {
            Ok(mut config) => {
                if config.version < CONFIG_VERSION {
                    config.version = CONFIG_VERSION;
                }
                config.validate();
                config
            }
            Err(_) => {
                let backup = path.with_extension("json.bad");
                let _ = fs::rename(path, backup);
                Self::default()
            }
        }
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let text = serde_json::to_vec_pretty(self).unwrap_or_default();
        replace_file(path, &text)
    }

    pub fn validate(&mut self) {
        self.version = CONFIG_VERSION;

        if !matches!(self.mode.as_str(), "island" | "card" | "lyrics") {
            self.mode = "card".to_string();
        }
        let themes = [
            "dark-glass",
            "light",
            "minimal",
            "neon",
            "mono",
            "album-color",
        ];
        if !themes.contains(&self.theme.as_str()) {
            self.theme = "custom".to_string();
        }

        let a = &mut self.appearance;
        let fonts = [
            "Segoe UI Variable Display",
            "Segoe UI",
            "Bahnschrift",
            "Cascadia Mono",
            "Consolas",
            "Georgia",
        ];
        if !fonts.contains(&a.font_family.as_str()) {
            a.font_family = "Segoe UI Variable Display".to_string();
        }
        a.font_size = a.font_size.clamp(10, 28);
        a.font_weight = a.font_weight.clamp(300, 700);
        a.text_color = sanitize_hex(&a.text_color).unwrap_or_else(|| "#ECEAE1".to_string());
        a.accent_color = sanitize_hex(&a.accent_color).unwrap_or_else(|| "#F07A4F".to_string());
        a.background_color =
            sanitize_hex(&a.background_color).unwrap_or_else(|| "#0C1012".to_string());
        a.opacity = clamp_f64(a.opacity, 0.1, 1.0);
        a.radius = a.radius.min(40);
        if !matches!(a.effect.as_str(), "acrylic" | "mica" | "none") {
            a.effect = "acrylic".to_string();
        }
        if !matches!(a.lyric_animation.as_str(), "fade" | "slide" | "karaoke") {
            a.lyric_animation = "slide".to_string();
        }
        a.lyric_lines = a.lyric_lines.clamp(3, 5);
        if !matches!(a.align.as_str(), "left" | "center") {
            a.align = "left".to_string();
        }

        self.lyrics.offset_ms = clamp_i64(self.lyrics.offset_ms, -10_000, 10_000);
        if !matches!(self.lyrics.unsynced.as_str(), "static" | "hide") {
            self.lyrics.unsynced = "static".to_string();
        }

        self.behavior.idle_destroy_minutes = self.behavior.idle_destroy_minutes.clamp(1, 60);
        self.behavior.auto_hide_paused_seconds = self.behavior.auto_hide_paused_seconds.min(600);
        self.behavior.preferred_apps =
            sanitize_apps(std::mem::take(&mut self.behavior.preferred_apps));
        self.behavior.ignored_apps = sanitize_apps(std::mem::take(&mut self.behavior.ignored_apps));
        self.behavior.fade_after_seconds = self.behavior.fade_after_seconds.min(120);
        self.behavior.quick_peek_seconds = self.behavior.quick_peek_seconds.clamp(2, 30);

        self.hotkeys.toggle = self.hotkeys.toggle.trim().chars().take(64).collect();
        self.hotkeys.click_through = self.hotkeys.click_through.trim().chars().take(64).collect();
        self.hotkeys.offset_up = self.hotkeys.offset_up.trim().chars().take(64).collect();
        self.hotkeys.offset_down = self.hotkeys.offset_down.trim().chars().take(64).collect();
        self.hotkeys.mode = self.hotkeys.mode.trim().chars().take(64).collect();
        self.hotkeys.peek = self.hotkeys.peek.trim().chars().take(64).collect();
        self.hotkeys.settings = self.hotkeys.settings.trim().chars().take(64).collect();

        self.per_song_offsets.retain(|key, value| {
            if key.is_empty() || key.len() > 512 {
                return false;
            }
            *value = clamp_i64(*value, -10_000, 10_000);
            true
        });
        if self.per_song_offsets.len() > 2000 {
            let keys: Vec<String> = self.per_song_offsets.keys().take(1000).cloned().collect();
            for key in keys {
                self.per_song_offsets.remove(&key);
            }
        }

        if self.behavior.battery_saver {
            self.appearance.effect = "none".to_string();
            self.appearance.lyric_animation = "fade".to_string();
            self.appearance.reduce_motion = true;
            self.appearance.shadow = false;
            self.behavior.fade_after_seconds = 0;
            self.behavior.idle_destroy_minutes = self.behavior.idle_destroy_minutes.min(2);
            self.behavior.auto_hide_paused_seconds = self.behavior.auto_hide_paused_seconds.min(10);
        }
    }

    pub fn per_song_offset(&self, key: &str) -> i64 {
        self.per_song_offsets.get(key).copied().unwrap_or(0)
    }

    pub fn effective_offset(&self, key: &str) -> i64 {
        self.lyrics.offset_ms + self.per_song_offset(key)
    }
}

pub fn apply_theme(config: &mut Config, id: &str) -> bool {
    let a = &mut config.appearance;
    match id {
        "dark-glass" => {
            a.background_color = "#0C1012".to_string();
            a.text_color = "#ECEAE1".to_string();
            a.accent_color = "#F07A4F".to_string();
            a.opacity = 0.92;
            a.radius = 6;
            a.effect = "none".to_string();
            a.accent_from_art = false;
            a.border = true;
            a.shadow = false;
        }
        "light" => {
            a.background_color = "#F3F1EA".to_string();
            a.text_color = "#171A19".to_string();
            a.accent_color = "#B23F26".to_string();
            a.opacity = 0.96;
            a.radius = 6;
            a.effect = "none".to_string();
            a.accent_from_art = false;
            a.border = true;
            a.shadow = false;
        }
        "minimal" => {
            a.background_color = "#141414".to_string();
            a.text_color = "#E8E6DF".to_string();
            a.accent_color = "#C7C2B6".to_string();
            a.opacity = 0.92;
            a.radius = 6;
            a.effect = "none".to_string();
            a.accent_from_art = false;
            a.border = true;
            a.shadow = false;
        }
        "neon" => {
            a.background_color = "#1A1110".to_string();
            a.text_color = "#F0E4D8".to_string();
            a.accent_color = "#E07A4F".to_string();
            a.opacity = 0.92;
            a.radius = 6;
            a.effect = "none".to_string();
            a.accent_from_art = false;
            a.border = true;
            a.shadow = false;
        }
        "mono" => {
            a.background_color = "#0E1416".to_string();
            a.text_color = "#E3ECE9".to_string();
            a.accent_color = "#9FC7BC".to_string();
            a.opacity = 0.92;
            a.radius = 6;
            a.effect = "none".to_string();
            a.accent_from_art = false;
            a.border = true;
            a.shadow = false;
        }
        "album-color" => {
            a.background_color = "#0C1012".to_string();
            a.text_color = "#ECEAE1".to_string();
            a.accent_color = "#F07A4F".to_string();
            a.opacity = 0.9;
            a.radius = 6;
            a.effect = "none".to_string();
            a.accent_from_art = true;
            a.border = true;
            a.shadow = false;
        }
        _ => return false,
    }
    a.validate_colors();
    config.theme = id.to_string();
    config.validate();
    true
}

impl Appearance {
    fn validate_colors(&mut self) {
        self.text_color = sanitize_hex(&self.text_color).unwrap_or_else(|| "#ECEAE1".to_string());
        self.accent_color =
            sanitize_hex(&self.accent_color).unwrap_or_else(|| "#F07A4F".to_string());
        self.background_color =
            sanitize_hex(&self.background_color).unwrap_or_else(|| "#0C1012".to_string());
    }
}

pub struct ConfigStore {
    pub path: PathBuf,
    pub sender: Mutex<Sender<Config>>,
}

impl ConfigStore {
    pub fn new(path: PathBuf) -> Self {
        let (tx, rx) = mpsc::channel::<Config>();
        let writer_path = path.clone();
        thread::Builder::new()
            .name("iceberg-config-writer".into())
            .spawn(move || writer_loop(writer_path, rx))
            .ok();
        Self {
            path,
            sender: Mutex::new(tx),
        }
    }

    pub fn queue(&self, config: Config) {
        if let Ok(sender) = self.sender.lock() {
            let _ = sender.send(config);
        }
    }

    pub fn flush(&self, config: &Config) {
        let _ = config.save(&self.path);
    }
}

fn writer_loop(path: PathBuf, rx: Receiver<Config>) {
    loop {
        let first = match rx.recv() {
            Ok(config) => config,
            Err(_) => return,
        };
        let mut latest = first;
        loop {
            match rx.recv_timeout(Duration::from_millis(300)) {
                Ok(config) => latest = config,
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => {
                    let _ = latest.save(&path);
                    return;
                }
            }
        }
        let _ = latest.save(&path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_values_are_clamped_and_sanitized() {
        let mut config = Config::default();
        config.appearance.opacity = 4.0;
        config.appearance.radius = 999;
        config.appearance.font_size = 1;
        config.appearance.font_family = "Comic Sans; url(evil)".to_string();
        config.appearance.text_color = "not-a-color".to_string();
        config.appearance.accent_color = "also-not-a-color".to_string();
        config.appearance.background_color = "still-not-a-color".to_string();
        config.lyrics.offset_ms = 99_999;
        config.behavior.idle_destroy_minutes = 0;
        config.mode = "nonsense".to_string();
        config.behavior.preferred_apps = vec![" spotify.exe ".to_string(), String::new()];
        config.validate();

        assert_eq!(config.appearance.opacity, 1.0);
        assert_eq!(config.appearance.radius, 40);
        assert_eq!(config.appearance.font_size, 10);
        assert_eq!(config.appearance.font_family, "Segoe UI Variable Display");
        assert_eq!(config.appearance.text_color, "#ECEAE1");
        assert_eq!(config.appearance.accent_color, "#F07A4F");
        assert_eq!(config.appearance.background_color, "#0C1012");
        assert_eq!(config.lyrics.offset_ms, 10_000);
        assert_eq!(config.behavior.idle_destroy_minutes, 1);
        assert_eq!(config.mode, "card");
        assert_eq!(config.behavior.preferred_apps, vec!["spotify.exe"]);
    }

    #[test]
    fn offsets_add_up() {
        let mut config = Config::default();
        config.lyrics.offset_ms = 200;
        config.per_song_offsets.insert("song".to_string(), 150);
        assert_eq!(config.effective_offset("song"), 350);
        assert_eq!(config.effective_offset("other"), 200);
    }

    #[test]
    fn battery_saver_turns_off_expensive_effects() {
        let mut config = Config::default();
        config.appearance.effect = "acrylic".to_string();
        config.appearance.lyric_animation = "karaoke".to_string();
        config.behavior.battery_saver = true;
        config.validate();
        assert_eq!(config.appearance.effect, "none");
        assert_eq!(config.appearance.lyric_animation, "fade");
        assert!(config.appearance.reduce_motion);
    }

    #[test]
    fn themes_apply_known_ids_only() {
        let mut config = Config::default();
        assert!(apply_theme(&mut config, "neon"));
        assert_eq!(config.theme, "neon");
        assert_eq!(config.appearance.accent_color, "#e07a4f");
        assert!(!apply_theme(&mut config, "missing"));
    }

    #[test]
    fn default_theme_is_glacial() {
        let config = Config::default();
        assert_eq!(config.appearance.background_color, "#0C1012");
        assert_eq!(config.appearance.text_color, "#ECEAE1");
        assert_eq!(config.appearance.accent_color, "#F07A4F");
        assert_eq!(config.appearance.radius, 6);
        assert!(!config.appearance.accent_from_art);
    }

    #[test]
    fn missing_fields_fall_back_to_defaults() {
        let config: Config = serde_json::from_str(r#"{"mode":"island"}"#).unwrap();
        assert_eq!(config.mode, "island");
        assert_eq!(config.appearance.font_size, 14);
        assert_eq!(config.version, CONFIG_VERSION);
    }
}
