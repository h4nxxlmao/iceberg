use serde::{Deserialize, Serialize};

use crate::config::{
    Align, Background, BackgroundKind, Config, TextCase, TransitionStyle, Typography,
};

/// A visual preset is a named background + typography combination. Position and lyric
/// offset are intentionally not part of a preset.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Preset {
    pub id: String,
    pub name: String,
    pub background: Background,
    pub typography: Typography,
}

impl Preset {
    pub fn apply_to(&self, config: &mut Config) {
        config.background = self.background.clone();
        config.typography = self.typography.clone();
        config.active_preset = Some(self.id.clone());
        config.validate();
    }
}

fn typography(font_size: f64, active_color: &str, inactive_opacity: f64) -> Typography {
    Typography {
        font_size,
        active_color: active_color.to_string(),
        inactive_opacity,
        ..Typography::default()
    }
}

fn background(kind: BackgroundKind, solid_color: &str) -> Background {
    Background {
        kind,
        solid_color: solid_color.to_string(),
        ..Background::default()
    }
}

pub fn builtin_presets() -> Vec<Preset> {
    vec![
        Preset {
            id: "ink".to_string(),
            name: "Ink".to_string(),
            background: background(BackgroundKind::Solid, "#0b0f11"),
            typography: typography(19.0, "#f1ede3", 0.34),
        },
        Preset {
            id: "paper".to_string(),
            name: "Paper".to_string(),
            background: Background {
                kind: BackgroundKind::Solid,
                solid_color: "#ece4d5".to_string(),
                solid_opacity: 0.98,
                border_color: "#3c392f".to_string(),
                border_opacity: 0.08,
                shadow: true,
                shadow_opacity: 0.18,
                ..Background::default()
            },
            typography: Typography {
                font_family: "Lato".to_string(),
                font_size: 18.5,
                weight: 400,
                letter_spacing: -0.005,
                line_height: 1.42,
                active_color: "#20211d".to_string(),
                inactive_opacity: 0.46,
                shadow: false,
                ..typography(18.5, "#20211d", 0.46)
            },
        },
        Preset {
            id: "clear".to_string(),
            name: "Clear".to_string(),
            background: Background {
                kind: BackgroundKind::Transparent,
                transparent_opacity: 0.0,
                border_width: 0.0,
                shadow: false,
                ..Background::default()
            },
            typography: Typography {
                font_family: "Barlow Condensed".to_string(),
                font_size: 20.0,
                weight: 500,
                line_height: 1.30,
                align: Align::Center,
                active_color: "#f8f4ea".to_string(),
                inactive_opacity: 0.40,
                shadow: true,
                outline: true,
                ..typography(20.0, "#f8f4ea", 0.40)
            },
        },
        Preset {
            id: "frost".to_string(),
            name: "Frost".to_string(),
            background: Background {
                kind: BackgroundKind::Ice,
                ice_frost: 0.72,
                ice_clarity: 0.42,
                ice_tint: "#c8dde2".to_string(),
                border_color: "#dbeeff".to_string(),
                border_opacity: 0.22,
                ..Background::default()
            },
            typography: Typography {
                font_size: 18.0,
                weight: 500,
                align: Align::Center,
                active_color: "#162024".to_string(),
                inactive_opacity: 0.44,
                shadow: false,
                ..typography(18.0, "#162024", 0.44)
            },
        },
        Preset {
            id: "glass".to_string(),
            name: "Glass".to_string(),
            background: Background {
                kind: BackgroundKind::Liquid,
                liquid_refraction: 0.62,
                liquid_blur: 22.0,
                liquid_highlight: 0.38,
                liquid_bevel: 20.0,
                liquid_chromatic: false,
                border_color: "#ffffff".to_string(),
                border_opacity: 0.18,
                ..Background::default()
            },
            typography: Typography {
                font_family: "Varela Round".to_string(),
                font_size: 18.0,
                weight: 400,
                line_height: 1.38,
                align: Align::Center,
                active_color: "#fbf8ef".to_string(),
                inactive_opacity: 0.38,
                ..typography(18.0, "#fbf8ef", 0.38)
            },
        },
        Preset {
            id: "quiet".to_string(),
            name: "Quiet".to_string(),
            background: Background {
                kind: BackgroundKind::Solid,
                solid_color: "#111314".to_string(),
                solid_opacity: 0.88,
                border_width: 0.0,
                shadow: false,
                ..Background::default()
            },
            typography: Typography {
                font_family: "Space Mono".to_string(),
                font_size: 16.0,
                weight: 400,
                line_height: 1.55,
                align: Align::Left,
                case: TextCase::Original,
                active_color: "#dcd8ce".to_string(),
                inactive_opacity: 0.30,
                shadow: false,
                transition: TransitionStyle::Fade,
                transition_ms: 180,
                ..typography(16.0, "#dcd8ce", 0.30)
            },
        },
    ]
}

pub fn find_preset(id: &str) -> Option<Preset> {
    builtin_presets().into_iter().find(|preset| preset.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn there_are_six_unique_builtins() {
        let presets = builtin_presets();
        assert_eq!(presets.len(), 6);
        let mut ids: Vec<&str> = presets.iter().map(|preset| preset.id.as_str()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 6);
    }

    #[test]
    fn applying_a_preset_keeps_position_and_offsets() {
        let mut config = Config::default();
        config.layout.offset_x = 123;
        config.set_song_offset("song", 200);
        let preset = find_preset("paper").unwrap();
        preset.apply_to(&mut config);
        assert_eq!(config.active_preset.as_deref(), Some("paper"));
        assert_eq!(config.layout.offset_x, 123);
        assert_eq!(config.effective_offset("song"), 200);
        assert_eq!(config.typography.font_family, "Lato");
    }

    #[test]
    fn presets_roundtrip() {
        let preset = find_preset("glass").unwrap();
        let json = serde_json::to_string(&preset).unwrap();
        let decoded: Preset = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.id, "glass");
        assert_eq!(decoded.background.kind, BackgroundKind::Liquid);
    }
}
