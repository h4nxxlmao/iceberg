use serde::{Deserialize, Serialize};
use std::path::Path;

pub const MAX_FONT_BYTES: u64 = 12 * 1024 * 1024;

/// Neutral reference metrics so every imported family renders at a comparable optical size.
/// These are close to the Windows UI font and do not depend on a font being installed.
pub const REFERENCE_CAP_RATIO: f32 = 0.700;
pub const REFERENCE_ASCENDER_RATIO: f32 = 0.930;
pub const REFERENCE_LINE_HEIGHT: f32 = 1.200;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct FontAxis {
    pub tag: String,
    pub min: f32,
    pub default: f32,
    pub max: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct FontInfo {
    pub family: String,
    pub subfamily: String,
    pub postscript_name: Option<String>,
    pub units_per_em: u16,
    pub ascender: i16,
    pub descender: i16,
    pub line_gap: i16,
    pub cap_height: Option<i16>,
    pub x_height: Option<i16>,
    pub weight: u16,
    pub italic: bool,
    pub variable_axes: Vec<FontAxis>,
}

impl FontInfo {
    pub fn cap_ratio(&self) -> f32 {
        let units = self.units_per_em.max(1) as f32;
        self.cap_height
            .map(|value| value as f32 / units)
            .unwrap_or(REFERENCE_CAP_RATIO)
    }

    pub fn x_ratio(&self) -> f32 {
        let units = self.units_per_em.max(1) as f32;
        self.x_height
            .map(|value| value as f32 / units)
            .unwrap_or(0.52)
    }

    pub fn ascender_ratio(&self) -> f32 {
        let units = self.units_per_em.max(1) as f32;
        self.ascender as f32 / units
    }

    pub fn descender_ratio(&self) -> f32 {
        let units = self.units_per_em.max(1) as f32;
        self.descender as f32 / units
    }

    pub fn natural_line_height_ratio(&self) -> f32 {
        let units = self.units_per_em.max(1) as f32;
        ((self.ascender as f32 - self.descender as f32 + self.line_gap as f32) / units)
            .clamp(0.8, 2.5)
    }
}

/// Values the renderer applies so a family sits on the same baseline and optical size as
/// the rest of the stock set.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct FontMetrics {
    pub size_multiplier: f32,
    pub baseline_offset_em: f32,
    pub line_height_multiplier: f32,
}

pub fn metrics_for(info: &FontInfo) -> FontMetrics {
    let cap_ratio = info.cap_ratio().max(0.35);
    let ascender_ratio = info.ascender_ratio();
    let natural = info.natural_line_height_ratio();
    let size_multiplier = (REFERENCE_CAP_RATIO / cap_ratio).clamp(0.5, 2.0);
    let baseline_offset_em =
        (REFERENCE_ASCENDER_RATIO - ascender_ratio * size_multiplier).clamp(-0.5, 0.5);
    let line_height_multiplier =
        (REFERENCE_LINE_HEIGHT / (natural * size_multiplier)).clamp(0.6, 2.0);
    FontMetrics {
        size_multiplier,
        baseline_offset_em,
        line_height_multiplier,
    }
}

pub fn load_font(path: &Path) -> Result<FontInfo, String> {
    let metadata = std::fs::metadata(path).map_err(|error| error.to_string())?;
    if metadata.len() == 0 {
        return Err("The font file is empty".to_string());
    }
    if metadata.len() > MAX_FONT_BYTES {
        return Err("The font file is too large".to_string());
    }
    let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
    parse_font(&bytes)
}

pub fn parse_font(bytes: &[u8]) -> Result<FontInfo, String> {
    if bytes.is_empty() {
        return Err("The font file is empty".to_string());
    }
    if bytes.len() as u64 > MAX_FONT_BYTES {
        return Err("The font file is too large".to_string());
    }
    let face = ttf_parser::Face::parse(bytes, 0)
        .map_err(|error| format!("The font could not be read: {error}"))?;
    let family = name_for(&face, 16)
        .or_else(|| name_for(&face, 1))
        .unwrap_or_else(|| "Unknown family".to_string());
    let subfamily = name_for(&face, 17)
        .or_else(|| name_for(&face, 2))
        .unwrap_or_else(|| "Regular".to_string());
    let postscript_name = name_for(&face, 6);
    let variable_axes = face
        .variation_axes()
        .into_iter()
        .map(|axis| FontAxis {
            tag: axis_tag(axis.tag),
            min: axis.min_value,
            default: axis.def_value,
            max: axis.max_value,
        })
        .collect();
    Ok(FontInfo {
        family,
        subfamily,
        postscript_name,
        units_per_em: face.units_per_em(),
        ascender: face.ascender(),
        descender: face.descender(),
        line_gap: face.line_gap(),
        cap_height: face.capital_height(),
        x_height: face.x_height(),
        weight: face.weight().to_number(),
        italic: face.is_italic() || face.is_oblique(),
        variable_axes,
    })
}

fn name_for(face: &ttf_parser::Face<'_>, id: u16) -> Option<String> {
    face.names()
        .into_iter()
        .find(|name| name.name_id == id)
        .and_then(|name| name.to_string())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn axis_tag(tag: ttf_parser::Tag) -> String {
    let bytes = tag.0.to_be_bytes();
    String::from_utf8_lossy(&bytes).trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(cap: i16, ascender: i16, descender: i16, line_gap: i16) -> FontInfo {
        FontInfo {
            family: "Test".to_string(),
            subfamily: "Regular".to_string(),
            postscript_name: None,
            units_per_em: 1000,
            ascender,
            descender,
            line_gap,
            cap_height: Some(cap),
            x_height: Some(500),
            weight: 400,
            italic: false,
            variable_axes: Vec::new(),
        }
    }

    #[test]
    fn reference_sized_font_needs_no_override() {
        let metrics = metrics_for(&sample(700, 930, -230, 0));
        assert!((metrics.size_multiplier - 1.0).abs() < 0.001);
        assert!(metrics.baseline_offset_em.abs() < 0.001);
        assert!(metrics.line_height_multiplier > 0.9);
    }

    #[test]
    fn smaller_cap_height_is_scaled_up() {
        let metrics = metrics_for(&sample(500, 800, -200, 0));
        assert!(metrics.size_multiplier > 1.2);
        assert!(metrics.size_multiplier <= 2.0);
    }

    #[test]
    fn missing_metrics_fall_back_safely() {
        let mut info = sample(700, 930, -230, 0);
        info.cap_height = None;
        info.x_height = None;
        let metrics = metrics_for(&info);
        assert!(metrics.size_multiplier.is_finite());
        assert!(metrics.baseline_offset_em.is_finite());
        assert!(metrics.line_height_multiplier.is_finite());
    }

    #[test]
    fn empty_font_is_rejected() {
        assert!(parse_font(&[]).is_err());
    }

    #[test]
    fn system_font_parses_when_available() {
        let path = Path::new(r"C:\Windows\Fonts\segoeui.ttf");
        if !path.exists() {
            return;
        }
        let info = load_font(path).expect("system font");
        assert!(!info.family.is_empty());
        assert!(info.units_per_em > 0);
        let metrics = metrics_for(&info);
        assert!(metrics.size_multiplier >= 0.5 && metrics.size_multiplier <= 2.0);
    }
}
