use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::Duration;

use crate::util::{clamp_f64, clamp_i64, replace_file, sanitize_apps, sanitize_hex};

pub const CONFIG_VERSION: u32 = 4;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Config {
    pub version: u32,
    pub mode: WidgetMode,
    pub background: Background,
    pub typography: Typography,
    pub layout: Layout,
    pub lyrics: LyricsSettings,
    pub behavior: Behavior,
    pub hotkeys: Hotkeys,
    pub per_song: HashMap<String, SongOverride>,
    pub active_preset: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WidgetMode {
    Island,
    #[default]
    Card,
    Lyrics,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceMode {
    #[default]
    Spotify,
    AppleMusic,
    Custom,
    Any,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackgroundKind {
    #[default]
    Solid,
    Transparent,
    Acrylic,
    Ice,
    Liquid,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackdropSource {
    #[default]
    Wallpaper,
    Live,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Align {
    #[default]
    Left,
    Center,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextCase {
    #[default]
    Original,
    Upper,
    Lower,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransitionStyle {
    #[default]
    Slide,
    Fade,
    Off,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Anchor {
    TopLeft,
    #[default]
    TopCenter,
    TopRight,
    CenterLeft,
    Center,
    CenterRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnsyncedMode {
    #[default]
    Static,
    Hide,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Background {
    pub kind: BackgroundKind,
    pub backdrop: BackdropSource,
    pub solid_color: String,
    pub solid_opacity: f64,
    pub transparent_opacity: f64,
    pub accent_color: String,
    pub accent_from_art: bool,
    pub acrylic_tint: String,
    pub acrylic_tint_strength: f64,
    pub acrylic_noise: f64,
    pub ice_frost: f64,
    pub ice_clarity: f64,
    pub ice_tint: String,
    pub liquid_refraction: f64,
    pub liquid_blur: f64,
    pub liquid_highlight: f64,
    pub liquid_bevel: f64,
    pub liquid_chromatic: bool,
    pub radius: u32,
    pub border_width: f64,
    pub border_color: String,
    pub border_opacity: f64,
    pub shadow: bool,
    pub shadow_opacity: f64,
    pub padding: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Typography {
    pub font_family: String,
    pub font_size: f64,
    pub weight: u32,
    pub italic: bool,
    pub letter_spacing: f64,
    pub line_height: f64,
    pub align: Align,
    pub case: TextCase,
    pub active_color: String,
    pub inactive_opacity: f64,
    pub show_marker: bool,
    pub marker_color: String,
    pub shadow: bool,
    pub outline: bool,
    pub visible_lines: u32,
    pub transition: TransitionStyle,
    pub transition_ms: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Layout {
    pub scale: u32,
    pub width: f64,
    pub art_size: f64,
    pub art_radius: f64,
    pub anchor: Anchor,
    pub offset_x: i32,
    pub offset_y: i32,
    pub per_monitor: HashMap<String, MonitorLayout>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct MonitorLayout {
    pub anchor: Anchor,
    pub offset_x: i32,
    pub offset_y: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct LyricsSettings {
    pub offset_ms: i64,
    pub unsynced: UnsyncedMode,
    pub word_highlight: bool,
    pub wrap: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Behavior {
    pub start_with_windows: bool,
    pub hide_on_fullscreen: bool,
    pub click_through: bool,
    pub edge_snap: bool,
    pub idle_timeout_seconds: u32,
    pub pause_hide_seconds: u32,
    pub battery_saver: bool,
    pub reduce_motion: bool,
    pub show_progress: bool,
    pub show_art: bool,
    pub show_meta: bool,
    pub config_transition_ms: u32,
    pub source: SourceMode,
    pub source_custom: String,
    pub preferred_apps: Vec<String>,
    pub ignored_apps: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
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

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct SongOverride {
    pub offset_ms: Option<i64>,
    pub font_family: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            mode: WidgetMode::Card,
            background: Background::default(),
            typography: Typography::default(),
            layout: Layout::default(),
            lyrics: LyricsSettings::default(),
            behavior: Behavior::default(),
            hotkeys: Hotkeys::default(),
            per_song: HashMap::new(),
            active_preset: Some("ink".to_string()),
        }
    }
}

impl Default for Background {
    fn default() -> Self {
        Self {
            kind: BackgroundKind::Solid,
            backdrop: BackdropSource::Wallpaper,
            solid_color: "#0b0f11".to_string(),
            solid_opacity: 0.94,
            transparent_opacity: 0.0,
            accent_color: "#f07a4f".to_string(),
            accent_from_art: false,
            acrylic_tint: "#10181c".to_string(),
            acrylic_tint_strength: 0.55,
            acrylic_noise: 0.06,
            ice_frost: 0.65,
            ice_clarity: 0.45,
            ice_tint: "#c8dde2".to_string(),
            liquid_refraction: 0.55,
            liquid_blur: 18.0,
            liquid_highlight: 0.35,
            liquid_bevel: 18.0,
            liquid_chromatic: false,
            radius: 8,
            border_width: 1.0,
            border_color: "#ffffff".to_string(),
            border_opacity: 0.12,
            shadow: true,
            shadow_opacity: 0.28,
            padding: 16.0,
        }
    }
}

impl Default for Typography {
    fn default() -> Self {
        Self {
            font_family: "Poppins".to_string(),
            font_size: 19.0,
            weight: 500,
            italic: false,
            letter_spacing: -0.012,
            line_height: 1.35,
            align: Align::Left,
            case: TextCase::Original,
            active_color: "#f1ede3".to_string(),
            inactive_opacity: 0.34,
            show_marker: true,
            marker_color: "#f1ede3".to_string(),
            shadow: true,
            outline: false,
            visible_lines: 5,
            transition: TransitionStyle::Slide,
            transition_ms: 520,
        }
    }
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            scale: 100,
            width: 360.0,
            art_size: 44.0,
            art_radius: 3.0,
            anchor: Anchor::TopCenter,
            offset_x: 0,
            offset_y: 24,
            per_monitor: HashMap::new(),
        }
    }
}

impl Default for MonitorLayout {
    fn default() -> Self {
        Self {
            anchor: Anchor::TopCenter,
            offset_x: 0,
            offset_y: 24,
        }
    }
}

impl Default for LyricsSettings {
    fn default() -> Self {
        Self {
            offset_ms: 0,
            unsynced: UnsyncedMode::Static,
            word_highlight: true,
            wrap: true,
        }
    }
}

impl Default for Behavior {
    fn default() -> Self {
        Self {
            start_with_windows: false,
            hide_on_fullscreen: true,
            click_through: false,
            edge_snap: true,
            idle_timeout_seconds: 300,
            pause_hide_seconds: 30,
            battery_saver: false,
            reduce_motion: false,
            show_progress: true,
            show_art: true,
            show_meta: true,
            config_transition_ms: 300,
            source: SourceMode::Spotify,
            source_custom: String::new(),
            preferred_apps: Vec::new(),
            ignored_apps: Vec::new(),
        }
    }
}

impl Default for Hotkeys {
    fn default() -> Self {
        Self {
            toggle: "Ctrl+Alt+I".to_string(),
            click_through: "Ctrl+Alt+L".to_string(),
            offset_up: "Ctrl+Alt+Right".to_string(),
            offset_down: "Ctrl+Alt+Left".to_string(),
            mode: "Ctrl+Alt+M".to_string(),
            peek: "Ctrl+Alt+P".to_string(),
            settings: "Ctrl+Alt+,".to_string(),
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
                config.migrate();
                config.validate();
                config
            }
            Err(_) => {
                let _ = fs::rename(path, path.with_extension("json.bad"));
                Self::default()
            }
        }
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let text = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
        replace_file(path, &text)
    }

    /// Fills fields that did not exist in older files. Serde already supplies defaults for
    /// missing fields; this is the place for value-level migrations.
    pub fn migrate(&mut self) {
        if self.version < 2 {
            self.typography.font_family = migrated_font_family(&self.typography.font_family);
            for song in self.per_song.values_mut() {
                if let Some(family) = &song.font_family {
                    song.font_family = Some(migrated_font_family(family));
                }
            }
        }
        if self.version < 3
            && self.active_preset.is_none()
            && self.typography.font_family.eq_ignore_ascii_case("lato")
        {
            self.typography.font_family = "Poppins".to_string();
            self.typography.font_size = 19.0;
            self.typography.line_height = 1.35;
            self.typography.letter_spacing = -0.012;
            self.typography.inactive_opacity = 0.34;
            self.typography.transition_ms = 360;
        }
        if self.version < 4 && self.active_preset.is_none() && self.typography.transition_ms == 360
        {
            self.typography.transition_ms = 520;
        }
        self.version = CONFIG_VERSION;
        if self.active_preset.as_deref() == Some("") {
            self.active_preset = None;
        }
    }

    pub fn validate(&mut self) {
        self.version = CONFIG_VERSION;
        if self.layout.per_monitor.len() > 16 {
            self.layout.per_monitor.clear();
        }

        let background = &mut self.background;
        background.solid_color =
            sanitize_hex(&background.solid_color).unwrap_or_else(|| "#0b0f11".to_string());
        background.accent_color =
            sanitize_hex(&background.accent_color).unwrap_or_else(|| "#f07a4f".to_string());
        background.acrylic_tint =
            sanitize_hex(&background.acrylic_tint).unwrap_or_else(|| "#10181c".to_string());
        background.ice_tint =
            sanitize_hex(&background.ice_tint).unwrap_or_else(|| "#c8dde2".to_string());
        background.border_color =
            sanitize_hex(&background.border_color).unwrap_or_else(|| "#ffffff".to_string());
        background.solid_opacity = clamp_f64(background.solid_opacity, 0.0, 1.0);
        background.transparent_opacity = clamp_f64(background.transparent_opacity, 0.0, 1.0);
        background.acrylic_tint_strength = clamp_f64(background.acrylic_tint_strength, 0.0, 1.0);
        background.acrylic_noise = clamp_f64(background.acrylic_noise, 0.0, 1.0);
        background.ice_frost = clamp_f64(background.ice_frost, 0.0, 1.0);
        background.ice_clarity = clamp_f64(background.ice_clarity, 0.0, 1.0);
        background.liquid_refraction = clamp_f64(background.liquid_refraction, 0.0, 1.0);
        background.liquid_blur = clamp_f64(background.liquid_blur, 0.0, 64.0);
        background.liquid_highlight = clamp_f64(background.liquid_highlight, 0.0, 1.0);
        background.liquid_bevel = clamp_f64(background.liquid_bevel, 0.0, 48.0);
        background.radius = background.radius.min(48);
        background.border_width = clamp_f64(background.border_width, 0.0, 4.0);
        background.border_opacity = clamp_f64(background.border_opacity, 0.0, 1.0);
        background.shadow_opacity = clamp_f64(background.shadow_opacity, 0.0, 1.0);
        background.padding = clamp_f64(background.padding, 4.0, 48.0);

        let typography = &mut self.typography;
        typography.font_family = typography.font_family.trim().chars().take(120).collect();
        if typography.font_family.is_empty() {
            typography.font_family = "Poppins".to_string();
        }
        typography.font_size = clamp_f64(typography.font_size, 12.0, 40.0);
        typography.weight = typography.weight.clamp(300, 800);
        typography.letter_spacing = clamp_f64(typography.letter_spacing, -0.05, 0.2);
        typography.line_height = clamp_f64(typography.line_height, 1.0, 2.0);
        typography.active_color =
            sanitize_hex(&typography.active_color).unwrap_or_else(|| "#f1ede3".to_string());
        typography.marker_color =
            sanitize_hex(&typography.marker_color).unwrap_or_else(|| "#f1ede3".to_string());
        typography.inactive_opacity = clamp_f64(typography.inactive_opacity, 0.15, 0.85);
        typography.visible_lines = typography.visible_lines.clamp(3, 7);
        typography.transition_ms = typography.transition_ms.clamp(100, 600);

        let layout = &mut self.layout;
        layout.scale = layout.scale.clamp(50, 200);
        layout.width = clamp_f64(layout.width, 240.0, 640.0);
        layout.art_size = clamp_f64(layout.art_size, 24.0, 72.0);
        layout.art_radius = clamp_f64(layout.art_radius, 0.0, 20.0);
        layout.offset_x = layout.offset_x.clamp(-10_000, 10_000);
        layout.offset_y = layout.offset_y.clamp(-10_000, 10_000);
        layout.per_monitor.retain(|name, monitor| {
            if name.is_empty() || name.len() > 120 {
                return false;
            }
            monitor.offset_x = monitor.offset_x.clamp(-10_000, 10_000);
            monitor.offset_y = monitor.offset_y.clamp(-10_000, 10_000);
            true
        });

        self.lyrics.offset_ms = clamp_i64(self.lyrics.offset_ms, -10_000, 10_000);

        self.behavior.idle_timeout_seconds = self.behavior.idle_timeout_seconds.min(3600);
        self.behavior.pause_hide_seconds = self.behavior.pause_hide_seconds.min(600);
        self.behavior.config_transition_ms = self.behavior.config_transition_ms.clamp(80, 800);
        self.behavior.source_custom = self
            .behavior
            .source_custom
            .trim()
            .chars()
            .take(120)
            .collect();
        self.behavior.preferred_apps =
            sanitize_apps(std::mem::take(&mut self.behavior.preferred_apps));
        self.behavior.ignored_apps = sanitize_apps(std::mem::take(&mut self.behavior.ignored_apps));

        for hotkey in [
            &mut self.hotkeys.toggle,
            &mut self.hotkeys.click_through,
            &mut self.hotkeys.offset_up,
            &mut self.hotkeys.offset_down,
            &mut self.hotkeys.mode,
            &mut self.hotkeys.peek,
            &mut self.hotkeys.settings,
        ] {
            *hotkey = hotkey.trim().chars().take(64).collect();
        }

        self.per_song.retain(|key, song| {
            if key.is_empty() || key.len() > 512 {
                return false;
            }
            if let Some(offset) = song.offset_ms {
                song.offset_ms = Some(clamp_i64(offset, -10_000, 10_000));
            }
            if let Some(family) = &song.font_family {
                let clean: String = family.trim().chars().take(120).collect();
                song.font_family = if clean.is_empty() { None } else { Some(clean) };
            }
            song.offset_ms.is_some() || song.font_family.is_some()
        });
        if self.per_song.len() > 2000 {
            let keys: Vec<String> = self.per_song.keys().take(1000).cloned().collect();
            for key in keys {
                self.per_song.remove(&key);
            }
        }

        if let Some(preset) = &self.active_preset {
            if preset.is_empty() || preset.len() > 80 {
                self.active_preset = None;
            }
        }
    }

    /// The config as the renderer should apply it. Battery saver and reduced motion are
    /// view-level overrides; the settings UI keeps showing the user's chosen values.
    pub fn effective(&self) -> Self {
        let mut config = self.clone();
        config.validate();
        if config.behavior.battery_saver {
            if config.background.kind != BackgroundKind::Transparent {
                config.background.kind = BackgroundKind::Solid;
            }
            config.typography.transition = TransitionStyle::Off;
            config.typography.shadow = false;
            config.typography.outline = false;
            config.background.shadow = false;
            config.behavior.reduce_motion = true;
        }
        if config.behavior.reduce_motion {
            config.typography.transition = TransitionStyle::Off;
        }
        config
    }

    pub fn effective_offset(&self, track_key: &str) -> i64 {
        self.lyrics.offset_ms + self.per_song_offset(track_key)
    }

    pub fn per_song_offset(&self, track_key: &str) -> i64 {
        self.per_song
            .get(track_key)
            .and_then(|song| song.offset_ms)
            .unwrap_or(0)
    }

    pub fn font_for(&self, track_key: &str) -> String {
        self.per_song
            .get(track_key)
            .and_then(|song| song.font_family.clone())
            .unwrap_or_else(|| self.typography.font_family.clone())
    }

    pub fn set_song_offset(&mut self, track_key: &str, offset: i64) {
        let entry = self.per_song.entry(track_key.to_string()).or_default();
        entry.offset_ms = if offset == 0 {
            None
        } else {
            Some(clamp_i64(offset, -10_000, 10_000))
        };
        if entry.offset_ms.is_none() && entry.font_family.is_none() {
            self.per_song.remove(track_key);
        }
    }

    pub fn set_song_font(&mut self, track_key: &str, family: Option<String>) {
        let entry = self.per_song.entry(track_key.to_string()).or_default();
        entry.font_family = family
            .map(|value| value.trim().chars().take(120).collect::<String>())
            .filter(|value| !value.is_empty());
        if entry.offset_ms.is_none() && entry.font_family.is_none() {
            self.per_song.remove(track_key);
        }
    }

    pub fn reset_song(&mut self, track_key: &str) {
        self.per_song.remove(track_key);
    }

    pub fn transition_duration(&self) -> Duration {
        Duration::from_millis(self.typography.transition_ms as u64)
    }
}

fn migrated_font_family(family: &str) -> String {
    match family.trim() {
        "Segoe UI Variable Display" | "Segoe UI Variable Text" | "Segoe UI" => "Lato".to_string(),
        "Georgia" => "Abril Fatface".to_string(),
        "Bahnschrift" => "Barlow Condensed".to_string(),
        "Consolas" | "Cascadia Mono" | "Cascadia Code" => "Space Mono".to_string(),
        other => other.to_string(),
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_valid() {
        let mut config = Config::default();
        config.validate();
        assert_eq!(config.version, CONFIG_VERSION);
        assert_eq!(config.mode, WidgetMode::Card);
        assert_eq!(config.background.kind, BackgroundKind::Solid);
        assert_eq!(config.typography.inactive_opacity, 0.34);
        assert_eq!(config.typography.font_family, "Poppins");
        assert_eq!(config.typography.font_size, 19.0);
        assert_eq!(config.layout.scale, 100);
        assert_eq!(config.lyrics.offset_ms, 0);
        assert!(config.effective().typography.transition != TransitionStyle::Off);
    }

    #[test]
    fn version_one_fonts_move_to_the_stock_set() {
        let mut config = Config::default();
        config.version = 1;
        config.typography.font_family = "Segoe UI Variable Display".to_string();
        config.set_song_font("song", Some("Georgia".to_string()));
        config.migrate();
        assert_eq!(config.version, CONFIG_VERSION);
        assert_eq!(config.typography.font_family, "Lato");
        assert_eq!(config.font_for("song"), "Abril Fatface");
    }

    #[test]
    fn invalid_values_are_clamped_and_sanitized() {
        let mut config = Config::default();
        config.background.solid_opacity = 4.0;
        config.background.radius = 999;
        config.background.border_color = "not-a-color".to_string();
        config.background.accent_color = "also-not-a-color".to_string();
        config.background.border_width = 99.0;
        config.typography.font_family = "   ".to_string();
        config.typography.font_size = 1.0;
        config.typography.weight = 99;
        config.typography.inactive_opacity = 9.0;
        config.typography.visible_lines = 99;
        config.typography.transition_ms = 9999;
        config.typography.active_color = "javascript:alert(1)".to_string();
        config.layout.scale = 999;
        config.layout.width = 10.0;
        config.layout.offset_x = 99_999;
        config.lyrics.offset_ms = 99_999;
        config.behavior.idle_timeout_seconds = 99_999;
        config.behavior.preferred_apps = vec![" spotify.exe ".to_string(), String::new()];
        config.hotkeys.toggle = "  Ctrl+Alt+I  ".to_string();
        config.validate();

        assert_eq!(config.background.solid_opacity, 1.0);
        assert_eq!(config.background.radius, 48);
        assert_eq!(config.background.border_color, "#ffffff");
        assert_eq!(config.background.accent_color, "#f07a4f");
        assert_eq!(config.background.border_width, 4.0);
        assert_eq!(config.typography.font_family, "Poppins");
        assert_eq!(config.typography.font_size, 12.0);
        assert_eq!(config.typography.weight, 300);
        assert_eq!(config.typography.inactive_opacity, 0.85);
        assert_eq!(config.typography.visible_lines, 7);
        assert_eq!(config.typography.transition_ms, 600);
        assert_eq!(config.typography.active_color, "#f1ede3");
        assert_eq!(config.layout.scale, 200);
        assert_eq!(config.layout.width, 240.0);
        assert_eq!(config.layout.offset_x, 10_000);
        assert_eq!(config.lyrics.offset_ms, 10_000);
        assert_eq!(config.behavior.idle_timeout_seconds, 3600);
        assert_eq!(config.behavior.preferred_apps, vec!["spotify.exe"]);
        assert_eq!(config.hotkeys.toggle, "Ctrl+Alt+I");
    }

    #[test]
    fn offsets_add_up() {
        let mut config = Config::default();
        config.lyrics.offset_ms = 200;
        config.set_song_offset("song", 150);
        assert_eq!(config.effective_offset("song"), 350);
        assert_eq!(config.effective_offset("other"), 200);
        config.set_song_offset("song", 0);
        assert!(!config.per_song.contains_key("song"));
    }

    #[test]
    fn per_song_font_is_separate_from_offset() {
        let mut config = Config::default();
        config.set_song_font("song", Some("Georgia".to_string()));
        assert_eq!(config.font_for("song"), "Georgia");
        config.set_song_offset("song", 100);
        assert_eq!(config.font_for("song"), "Georgia");
        config.set_song_font("song", None);
        assert_eq!(config.font_for("song"), config.typography.font_family);
        assert!(config.per_song.contains_key("song"));
    }

    #[test]
    fn battery_saver_overrides_only_the_effective_view() {
        let mut config = Config::default();
        config.background.kind = BackgroundKind::Liquid;
        config.typography.transition = TransitionStyle::Slide;
        config.behavior.battery_saver = true;
        let effective = config.effective();
        assert_eq!(effective.background.kind, BackgroundKind::Solid);
        assert_eq!(effective.typography.transition, TransitionStyle::Off);
        assert!(effective.behavior.reduce_motion);
        assert_eq!(config.background.kind, BackgroundKind::Liquid);
        assert_eq!(config.typography.transition, TransitionStyle::Slide);
    }

    #[test]
    fn transparent_survives_battery_saver() {
        let mut config = Config::default();
        config.background.kind = BackgroundKind::Transparent;
        config.behavior.battery_saver = true;
        assert_eq!(
            config.effective().background.kind,
            BackgroundKind::Transparent
        );
    }

    #[test]
    fn missing_fields_fall_back_to_defaults() {
        let config: Config = serde_json::from_str(r#"{"mode":"island"}"#).unwrap();
        assert_eq!(config.mode, WidgetMode::Island);
        assert_eq!(config.typography.font_size, 19.0);
        assert_eq!(config.version, CONFIG_VERSION);
    }

    #[test]
    fn version_two_unpinned_lato_moves_to_poppins() {
        let mut config = Config::default();
        config.version = 2;
        config.active_preset = None;
        config.typography.font_family = "Lato".to_string();
        config.migrate();
        assert_eq!(config.typography.font_family, "Poppins");
        assert_eq!(config.typography.font_size, 19.0);
        assert_eq!(config.typography.transition_ms, 520);
    }

    #[test]
    fn roundtrip_and_corrupt_file_backup() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("config.json");
        let mut config = Config::default();
        config.mode = WidgetMode::Lyrics;
        config.typography.font_family = "Georgia".to_string();
        config.save(&path).unwrap();

        let loaded = Config::load(&path);
        assert_eq!(loaded.mode, WidgetMode::Lyrics);
        assert_eq!(loaded.typography.font_family, "Georgia");

        fs::write(&path, "{ not json").unwrap();
        let loaded = Config::load(&path);
        assert_eq!(loaded.mode, WidgetMode::Card);
        assert!(path.with_extension("json.bad").exists());
    }

    #[test]
    fn older_version_is_migrated() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("config.json");
        fs::write(&path, r#"{"version":0,"mode":"card"}"#).unwrap();
        let loaded = Config::load(&path);
        assert_eq!(loaded.version, CONFIG_VERSION);
    }
}
