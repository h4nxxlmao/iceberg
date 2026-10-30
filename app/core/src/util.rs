use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

pub fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in bytes {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

pub fn cache_name(key: &str) -> String {
    format!("{:016x}", fnv1a(key.as_bytes()))
}

pub fn normalize_component(input: &str) -> String {
    let lower = input.to_lowercase();
    let mut out = String::with_capacity(lower.len());
    let mut last_space = false;
    for character in lower.chars() {
        if character.is_alphanumeric() {
            out.push(character);
            last_space = false;
        } else if !last_space {
            out.push(' ');
            last_space = true;
        }
    }
    out.trim().to_string()
}

fn bracket_is_noise(inner: &str) -> bool {
    const NOISE: &[&str] = &[
        "official",
        "lyric",
        "audio",
        "video",
        "visualiz",
        "visualis",
        "remaster",
        "topic",
        "explicit",
        "clean version",
        "feat.",
        "ft.",
        "prod.",
        "color coded",
        "sped up",
        "slowed",
    ];
    if NOISE.iter().any(|needle| inner.contains(needle)) {
        return true;
    }
    const SHORT: &[&str] = &["hd", "hq", "4k", "8d"];
    inner
        .split(|character: char| !character.is_alphanumeric())
        .any(|token| SHORT.contains(&token))
}

/// Removes the usual SMTC title decorations without touching meaningful parentheses.
pub fn clean_title(raw: &str) -> String {
    let characters: Vec<char> = raw.chars().collect();
    let mut out = String::with_capacity(raw.len());
    let mut inner = String::new();
    let mut open: Option<char> = None;

    for &character in &characters {
        match open {
            None => {
                if character == '(' || character == '[' || character == '{' {
                    open = Some(character);
                    inner.clear();
                } else {
                    out.push(character);
                }
            }
            Some(bracket) => {
                let closes = matches!((bracket, character), ('(', ')') | ('[', ']') | ('{', '}'));
                if closes {
                    let lower = inner.to_lowercase();
                    if !bracket_is_noise(&lower) {
                        out.push(bracket);
                        out.push_str(&inner);
                        out.push(match bracket {
                            '(' => ')',
                            '[' => ']',
                            _ => '}',
                        });
                    }
                    open = None;
                } else {
                    inner.push(character);
                }
            }
        }
    }

    if let Some(bracket) = open {
        out.push(bracket);
        out.push_str(&inner);
    }

    let mut segments: Vec<String> = out
        .split('|')
        .map(|segment| segment.trim().to_string())
        .filter(|segment| !segment.is_empty())
        .collect();
    segments.retain(|segment| !bracket_is_noise(&segment.to_lowercase()));
    let mut text = segments
        .first()
        .cloned()
        .unwrap_or_else(|| out.trim().to_string());

    const SUFFIXES: &[&str] = &[
        " - topic",
        " - official video",
        " - official audio",
        " - official music video",
        " - music video",
        " - lyrics",
        " - lyric video",
        " - visualizer",
        " - audio",
        " - hd",
        " - hq",
    ];
    loop {
        let lower = text.to_lowercase();
        let mut changed = false;
        for suffix in SUFFIXES {
            if lower.ends_with(suffix) {
                let keep = text.len().saturating_sub(suffix.len());
                text.truncate(keep);
                changed = true;
                break;
            }
        }
        if !changed {
            break;
        }
        text = text.trim().to_string();
    }

    let mut compact = String::with_capacity(text.len());
    let mut space = false;
    for character in text.chars() {
        if character.is_whitespace() {
            if !space {
                compact.push(' ');
                space = true;
            }
        } else {
            compact.push(character);
            space = false;
        }
    }
    let trimmed = compact.trim().trim_matches('-').trim();
    if trimmed.is_empty() {
        raw.trim().chars().take(200).collect()
    } else {
        trimmed.chars().take(200).collect()
    }
}

pub fn clean_artist(raw: &str) -> String {
    let mut text = raw.trim().to_string();
    if text.to_lowercase().ends_with(" - topic") {
        text.truncate(text.len() - 8);
    }
    if text.trim().is_empty() {
        "Unknown artist".to_string()
    } else {
        text.trim().chars().take(160).collect()
    }
}

/// Stable per-track id: cleaned artist/title/album plus duration in whole seconds.
pub fn track_key(title: &str, artist: &str, album: &str, duration_ms: i64) -> String {
    let seconds = (duration_ms.max(0) / 1000) as u64;
    format!(
        "{}|{}|{}|{}",
        normalize_component(&clean_title(title)),
        normalize_component(&clean_artist(artist)),
        normalize_component(album),
        seconds
    )
}

pub fn sanitize_hex(value: &str) -> Option<String> {
    let clean = value.trim();
    let body = clean.strip_prefix('#')?;
    if !body.chars().all(|character| character.is_ascii_hexdigit()) {
        return None;
    }
    match body.len() {
        3 | 6 => Some(format!("#{}", body.to_lowercase())),
        _ => None,
    }
}

pub fn clamp_i64(value: i64, min: i64, max: i64) -> i64 {
    value.max(min).min(max)
}

pub fn clamp_f64(value: f64, min: f64, max: f64) -> f64 {
    if value.is_nan() {
        min
    } else {
        value.max(min).min(max)
    }
}

pub fn sanitize_apps(values: Vec<String>) -> Vec<String> {
    let mut out = Vec::new();
    for value in values {
        let clean = value.trim();
        if clean.is_empty() {
            continue;
        }
        let limited: String = clean.chars().take(120).collect();
        if !out
            .iter()
            .any(|existing: &String| existing.eq_ignore_ascii_case(&limited))
        {
            out.push(limited);
        }
        if out.len() >= 32 {
            break;
        }
    }
    out
}

pub fn dir_size(path: &Path) -> u64 {
    let mut total = 0u64;
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    for entry in entries.flatten() {
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if metadata.is_dir() {
            total = total.saturating_add(dir_size(&entry.path()));
        } else if metadata.is_file() {
            total = total.saturating_add(metadata.len());
        }
    }
    total
}

pub fn replace_file(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension(format!(
        "tmp-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0)
    ));
    fs::write(&tmp, contents)?;
    match fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = fs::remove_file(&tmp);
            Err(error)
        }
    }
}

pub fn read_limited(path: &Path, limit: usize) -> std::io::Result<String> {
    let bytes = fs::read(path)?;
    let end = bytes.len().min(limit);
    Ok(String::from_utf8_lossy(&bytes[..end]).into_owned())
}

pub fn format_ms(milliseconds: i64) -> String {
    let seconds = (milliseconds.max(0) / 1000) as u64;
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_common_noise_from_titles() {
        assert_eq!(clean_title("Song (Official Video)"), "Song");
        assert_eq!(clean_title("Song [Official Audio]"), "Song");
        assert_eq!(clean_title("Song (Lyrics)"), "Song");
        assert_eq!(clean_title("Song - Topic"), "Song");
        assert_eq!(clean_title("Song (feat. Someone)"), "Song");
        assert_eq!(clean_title("Song (Official Video) | Artist | 4K"), "Song");
    }

    #[test]
    fn keeps_meaningful_parentheses() {
        assert_eq!(
            clean_title("(I Can't Get No) Satisfaction"),
            "(I Can't Get No) Satisfaction"
        );
        assert_eq!(
            clean_title("Song (Live at Wembley)"),
            "Song (Live at Wembley)"
        );
    }

    #[test]
    fn artist_topic_suffix_is_removed() {
        assert_eq!(clean_artist("Some Artist - Topic"), "Some Artist");
        assert_eq!(clean_artist("  "), "Unknown artist");
    }

    #[test]
    fn cache_keys_are_stable_and_case_insensitive() {
        let first = track_key("Midnight City", "M83", "Hurry Up", 244_000);
        let second = track_key("midnight city", "m83", "hurry up", 244_900);
        assert_eq!(first, second);
        assert_ne!(
            first,
            track_key("Midnight City", "M83", "Hurry Up", 200_000)
        );
    }

    #[test]
    fn colors_are_validated() {
        assert_eq!(sanitize_hex("#ABC"), Some("#abc".to_string()));
        assert_eq!(sanitize_hex("#a1b2c3"), Some("#a1b2c3".to_string()));
        assert_eq!(sanitize_hex("red"), None);
        assert_eq!(sanitize_hex("#12345"), None);
        assert_eq!(sanitize_hex("#ffff; background:url(x)"), None);
    }

    #[test]
    fn formatting_and_clamping_are_stable() {
        assert_eq!(format_ms(0), "0:00");
        assert_eq!(format_ms(62_000), "1:02");
        assert_eq!(clamp_i64(99, -10, 10), 10);
        assert_eq!(clamp_f64(f64::NAN, 0.0, 1.0), 0.0);
    }
}
