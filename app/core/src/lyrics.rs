use serde::Deserialize;
use std::io::Read;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use crate::cache;
use crate::model::{LyricLine, Lyrics, LyricsKind, SearchResult, Track, Word};
use crate::util::{clean_artist, clean_title};

const USER_AGENT: &str = "Iceberg/0.1.0 (https://github.com/h4nxxlmao/iceberg)";
const MAX_BODY: u64 = 2 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize)]
struct ApiTrack {
    #[serde(default)]
    id: u64,
    #[serde(rename = "trackName", default)]
    track_name: String,
    #[serde(rename = "artistName", default)]
    artist_name: String,
    #[serde(rename = "albumName", default)]
    album_name: Option<String>,
    #[serde(default)]
    duration: Option<f64>,
    #[serde(rename = "syncedLyrics", default)]
    synced_lyrics: Option<String>,
    #[serde(rename = "plainLyrics", default)]
    plain_lyrics: Option<String>,
    #[serde(default)]
    instrumental: Option<bool>,
}

impl ApiTrack {
    fn has_synced(&self) -> bool {
        self.synced_lyrics
            .as_deref()
            .map(|text| !text.trim().is_empty())
            .unwrap_or(false)
    }

    fn has_plain(&self) -> bool {
        self.plain_lyrics
            .as_deref()
            .map(|text| !text.trim().is_empty())
            .unwrap_or(false)
    }

    fn duration_seconds(&self) -> f64 {
        self.duration.unwrap_or(0.0)
    }

    fn is_instrumental(&self) -> bool {
        self.instrumental.unwrap_or(false)
    }

    fn search_result(&self) -> SearchResult {
        SearchResult {
            id: self.id,
            track_name: self.track_name.clone(),
            artist_name: self.artist_name.clone(),
            album_name: self.album_name.clone().unwrap_or_default(),
            duration: self.duration_seconds(),
            instrumental: self.is_instrumental(),
            has_synced: self.has_synced(),
            has_plain: self.has_plain(),
        }
    }
}

/// LRCLIB client plus the on-disk lyric cache. All network calls are HTTPS to lrclib.net
/// with a five second timeout and a two megabyte response cap.
pub struct LyricsService {
    agent: ureq::Agent,
    cache_dir: PathBuf,
}

impl LyricsService {
    pub fn new(cache_dir: PathBuf) -> Self {
        let _ = std::fs::create_dir_all(cache::lyrics_dir(&cache_dir));
        let _ = std::fs::create_dir_all(cache::art_dir(&cache_dir));
        let _ = std::fs::create_dir_all(cache::override_dir(&cache_dir));
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(5))
            .user_agent(USER_AGENT)
            .build();
        Self { agent, cache_dir }
    }

    pub fn cache_dir(&self) -> &std::path::Path {
        &self.cache_dir
    }

    pub fn fetch_for(&self, track: &Track, force: bool) -> Lyrics {
        if !force {
            if let Some(text) = cache::read_override(&self.cache_dir, &track.key) {
                return lyrics_from_manual(track, &text, "file");
            }
            if let Some(entry) = cache::read_lyrics(&self.cache_dir, &track.key) {
                if entry.not_found {
                    return none_lyrics(track, "cache");
                }
                if let Some(lyrics) = entry.lyrics {
                    return lyrics;
                }
            }
        }

        match self.fetch_network(track) {
            Ok(Some(api)) => {
                let lyrics = lyrics_from_api(track, &api);
                cache::write_lyrics(
                    &self.cache_dir,
                    &track.key,
                    Some(&lyrics),
                    lyrics.kind == LyricsKind::None,
                );
                lyrics
            }
            Ok(None) => {
                let lyrics = none_lyrics(track, "lrclib");
                cache::write_lyrics(&self.cache_dir, &track.key, None, true);
                lyrics
            }
            Err(error) => Lyrics {
                track_key: track.key.clone(),
                kind: LyricsKind::Error,
                lines: Vec::new(),
                plain: String::new(),
                source: "lrclib".to_string(),
                message: if error.to_lowercase().contains("network") {
                    "Offline or LRCLIB unreachable".to_string()
                } else {
                    "Lyrics unavailable".to_string()
                },
            },
        }
    }

    pub fn search_free(&self, query: &str) -> Result<Vec<SearchResult>, String> {
        let clean = query.trim();
        if clean.is_empty() {
            return Ok(Vec::new());
        }
        let params = vec![("q".to_string(), clean.chars().take(200).collect())];
        let items = self
            .http_json::<Vec<ApiTrack>>("https://lrclib.net/api/search", &params)?
            .unwrap_or_default();
        Ok(items.iter().take(20).map(ApiTrack::search_result).collect())
    }

    pub fn apply_choice(&self, track: &Track, id: u64) -> Result<Lyrics, String> {
        let api = self
            .api_get_by_id(id)?
            .ok_or("LRCLIB has no record for that id")?;
        let lyrics = lyrics_from_api(track, &api);
        if lyrics.kind == LyricsKind::None {
            return Err("That result has no lyrics".to_string());
        }
        let raw = api
            .synced_lyrics
            .as_deref()
            .filter(|text| !text.trim().is_empty())
            .or(api.plain_lyrics.as_deref())
            .unwrap_or("");
        if !raw.trim().is_empty() {
            cache::write_override(&self.cache_dir, &track.key, raw);
        }
        cache::write_lyrics(
            &self.cache_dir,
            &track.key,
            Some(&lyrics),
            lyrics.kind == LyricsKind::None,
        );
        Ok(lyrics)
    }

    pub fn import_override(&self, track: &Track, text: &str) -> Result<Lyrics, String> {
        let lyrics = lyrics_from_manual(track, text, "file");
        if lyrics.kind == LyricsKind::None {
            return Err("That file has no lyrics".to_string());
        }
        if !cache::write_override(&self.cache_dir, &track.key, text) {
            return Err("Could not write the lyric file".to_string());
        }
        cache::write_lyrics(&self.cache_dir, &track.key, Some(&lyrics), false);
        Ok(lyrics)
    }

    fn agent(&self) -> &ureq::Agent {
        &self.agent
    }

    fn read_body(&self, response: ureq::Response) -> Result<String, String> {
        let mut body = String::new();
        response
            .into_reader()
            .take(MAX_BODY)
            .read_to_string(&mut body)
            .map_err(|error| format!("Network error: {error}"))?;
        Ok(body)
    }

    fn http_json<T: for<'de> Deserialize<'de>>(
        &self,
        url: &str,
        query: &[(String, String)],
    ) -> Result<Option<T>, String> {
        let mut last_error = String::new();
        for attempt in 0..2 {
            let mut request = self.agent().get(url);
            for (key, value) in query {
                request = request.query(key, value);
            }
            match request.call() {
                Ok(response) => {
                    let body = self.read_body(response)?;
                    return serde_json::from_str::<T>(&body)
                        .map(Some)
                        .map_err(|error| format!("Bad response: {error}"));
                }
                Err(ureq::Error::Status(404, _)) => return Ok(None),
                Err(ureq::Error::Status(code, _)) if code >= 500 => {
                    last_error = format!("LRCLIB returned {code}");
                }
                Err(ureq::Error::Status(code, _)) => {
                    return Err(format!("LRCLIB returned {code}"));
                }
                Err(ureq::Error::Transport(error)) => {
                    last_error = format!("Network error: {error}");
                }
            }
            if attempt == 0 {
                thread::sleep(Duration::from_millis(350));
            }
        }
        Err(last_error)
    }

    fn api_get(&self, track: &Track) -> Result<Option<ApiTrack>, String> {
        let mut query = vec![
            ("track_name".to_string(), clean_title(&track.title)),
            ("artist_name".to_string(), clean_artist(&track.artist)),
        ];
        let album = track.album.trim();
        if !album.is_empty() {
            query.push(("album_name".to_string(), album.to_string()));
        }
        if track.duration_ms > 0 {
            query.push((
                "duration".to_string(),
                (track.duration_ms / 1000).to_string(),
            ));
        }
        self.http_json("https://lrclib.net/api/get", &query)
    }

    fn api_search(&self, track: &Track) -> Result<Vec<ApiTrack>, String> {
        let query = vec![
            ("track_name".to_string(), clean_title(&track.title)),
            ("artist_name".to_string(), clean_artist(&track.artist)),
        ];
        let mut items = self
            .http_json::<Vec<ApiTrack>>("https://lrclib.net/api/search", &query)?
            .unwrap_or_default();
        if items.is_empty() {
            // LRCLIB's structured search can miss decorated titles and featured artists.
            let free = format!(
                "{} {}",
                clean_artist(&track.artist),
                clean_title(&track.title)
            );
            let free = free.trim();
            if !free.is_empty() {
                items = self
                    .http_json::<Vec<ApiTrack>>(
                        "https://lrclib.net/api/search",
                        &[("q".to_string(), free.to_string())],
                    )?
                    .unwrap_or_default();
            }
        }
        Ok(items)
    }

    fn api_get_by_id(&self, id: u64) -> Result<Option<ApiTrack>, String> {
        self.http_json(&format!("https://lrclib.net/api/get/{id}"), &[])
    }

    fn fetch_network(&self, track: &Track) -> Result<Option<ApiTrack>, String> {
        if let Some(item) = self.api_get(track)? {
            if item.has_synced() || item.has_plain() || item.is_instrumental() {
                return Ok(Some(item));
            }
        }
        let results = self.api_search(track)?;
        let Some(best) = best_match(&results, track.duration_ms) else {
            return Ok(None);
        };
        if best.has_synced() || best.has_plain() || best.is_instrumental() {
            return Ok(Some(best.clone()));
        }
        if best.id > 0 {
            if let Some(detail) = self.api_get_by_id(best.id)? {
                return Ok(Some(detail));
            }
        }
        Ok(None)
    }
}

fn none_lyrics(track: &Track, source: &str) -> Lyrics {
    Lyrics {
        track_key: track.key.clone(),
        kind: LyricsKind::None,
        lines: Vec::new(),
        plain: String::new(),
        source: source.to_string(),
        message: "No lyrics found".to_string(),
    }
}

fn best_match(items: &[ApiTrack], duration_ms: i64) -> Option<&ApiTrack> {
    if items.is_empty() {
        return None;
    }
    let target = duration_ms as f64 / 1000.0;
    let mut best: Option<(&ApiTrack, f64, bool)> = None;
    for item in items {
        let difference = if item.duration.is_some() {
            (item.duration_seconds() - target).abs()
        } else {
            f64::INFINITY
        };
        let within = duration_ms <= 0 || difference <= 2.0;
        let score = (within, item.has_synced(), -difference);
        match best {
            None => best = Some((item, difference, within)),
            Some((current, current_difference, current_within)) => {
                let current_score = (current_within, current.has_synced(), -current_difference);
                if score > current_score {
                    best = Some((item, difference, within));
                }
            }
        }
    }
    best.map(|(item, _, _)| item)
}

fn parse_timestamp(tag: &str) -> Option<i64> {
    let (minutes, seconds) = tag.split_once(':')?;
    if seconds.contains(':') {
        return None;
    }
    let minutes: u64 = minutes.trim().parse().ok()?;
    let seconds: f64 = seconds.trim().replace(',', ".").parse().ok()?;
    if !seconds.is_finite() || seconds >= 60.0 {
        return None;
    }
    Some((minutes as f64 * 60_000.0 + seconds * 1000.0).round() as i64)
}

/// Parses LRC, including multiple timestamps per line and enhanced word timings.
pub fn parse_lrc(text: &str) -> Vec<LyricLine> {
    let mut lines: Vec<LyricLine> = Vec::new();
    let mut embedded_offset = 0i64;

    for raw in text.lines().take(6000) {
        let mut rest = raw.trim_start();
        let mut stamps: Vec<i64> = Vec::new();
        loop {
            if !rest.starts_with('[') {
                break;
            }
            let Some(end) = rest.find(']') else {
                break;
            };
            let tag = &rest[1..end];
            if let Some(value) = tag.strip_prefix("offset:") {
                embedded_offset = value.trim().parse().unwrap_or(0);
            } else if let Some(millis) = parse_timestamp(tag) {
                stamps.push(millis);
            }
            rest = &rest[end + 1..];
        }
        if stamps.is_empty() {
            continue;
        }
        let content = rest.trim();
        let fallback = stamps[0];
        let (line_text, mut words) = parse_word_tags(content, fallback);
        for word in &mut words {
            word.t_ms = (word.t_ms - embedded_offset).max(0);
        }
        for stamp in stamps {
            let adjusted = (stamp - embedded_offset).max(0);
            lines.push(LyricLine {
                t_ms: adjusted,
                text: line_text.chars().take(2000).collect(),
                words: words.clone(),
            });
        }
        if lines.len() > 5000 {
            break;
        }
    }

    lines.sort_by_key(|line| line.t_ms);
    lines.dedup_by(|a, b| a.t_ms == b.t_ms && a.text == b.text);
    lines
}

fn parse_word_tags(content: &str, fallback_t: i64) -> (String, Vec<Word>) {
    let mut segments: Vec<(Option<i64>, &str)> = Vec::new();
    let mut current: Option<i64> = None;
    let mut last = 0usize;
    let bytes = content.as_bytes();
    let mut index = 0usize;
    let mut saw_tag = false;
    while index < bytes.len() {
        if bytes[index] == b'<' {
            if let Some(end) = content[index + 1..].find('>') {
                let tag = &content[index + 1..index + 1 + end];
                if let Some(time) = parse_timestamp(tag) {
                    let segment = &content[last..index];
                    if !segment.is_empty() {
                        segments.push((current, segment));
                    }
                    current = Some(time);
                    last = index + 1 + end + 1;
                    index = last;
                    saw_tag = true;
                    continue;
                }
            }
        }
        index += 1;
    }
    if last < content.len() {
        segments.push((current, &content[last..]));
    }
    if !saw_tag {
        return (content.trim().to_string(), Vec::new());
    }
    let mut text = String::new();
    let mut words = Vec::new();
    for (time, segment) in segments {
        let clean = segment.trim();
        if clean.is_empty() {
            continue;
        }
        if !text.is_empty() {
            text.push(' ');
        }
        text.push_str(clean);
        words.push(Word {
            t_ms: time.unwrap_or(fallback_t),
            text: clean.to_string(),
        });
    }
    (text, words)
}

fn plain_text(value: Option<&str>) -> String {
    value
        .unwrap_or("")
        .trim()
        .chars()
        .take(50_000)
        .collect::<String>()
}

fn is_instrumental_text(text: &str) -> bool {
    let lower = text.to_lowercase();
    let trimmed = lower.trim();
    trimmed == "instrumental"
        || trimmed == "[instrumental]"
        || trimmed == "(instrumental)"
        || lower.contains("[instrumental]")
        || lower.contains("(instrumental)")
}

fn lyrics_from_api(track: &Track, api: &ApiTrack) -> Lyrics {
    if api.is_instrumental() {
        return Lyrics {
            track_key: track.key.clone(),
            kind: LyricsKind::Instrumental,
            lines: Vec::new(),
            plain: plain_text(api.plain_lyrics.as_deref()),
            source: "lrclib".to_string(),
            message: "Instrumental".to_string(),
        };
    }

    if let Some(synced) = api.synced_lyrics.as_deref() {
        if is_instrumental_text(synced) {
            return Lyrics {
                track_key: track.key.clone(),
                kind: LyricsKind::Instrumental,
                lines: Vec::new(),
                plain: plain_text(api.plain_lyrics.as_deref()),
                source: "lrclib".to_string(),
                message: "Instrumental".to_string(),
            };
        }
        let parsed = parse_lrc(synced);
        if !parsed.is_empty() {
            return Lyrics {
                track_key: track.key.clone(),
                kind: LyricsKind::Synced,
                lines: parsed,
                plain: plain_text(api.plain_lyrics.as_deref()),
                source: "lrclib".to_string(),
                message: String::new(),
            };
        }
    }

    let plain = plain_text(api.plain_lyrics.as_deref());
    if !plain.is_empty() {
        if is_instrumental_text(&plain) {
            return Lyrics {
                track_key: track.key.clone(),
                kind: LyricsKind::Instrumental,
                lines: Vec::new(),
                plain,
                source: "lrclib".to_string(),
                message: "Instrumental".to_string(),
            };
        }
        return Lyrics {
            track_key: track.key.clone(),
            kind: LyricsKind::Plain,
            lines: Vec::new(),
            plain,
            source: "lrclib".to_string(),
            message: String::new(),
        };
    }

    none_lyrics(track, "lrclib")
}

fn lyrics_from_manual(track: &Track, text: &str, source: &str) -> Lyrics {
    let parsed = parse_lrc(text);
    if !parsed.is_empty() {
        return Lyrics {
            track_key: track.key.clone(),
            kind: LyricsKind::Synced,
            lines: parsed,
            plain: String::new(),
            source: source.to_string(),
            message: String::new(),
        };
    }
    let plain = plain_text(Some(text));
    if plain.is_empty() {
        return Lyrics {
            track_key: track.key.clone(),
            kind: LyricsKind::None,
            lines: Vec::new(),
            plain: String::new(),
            source: source.to_string(),
            message: "Empty lyric file".to_string(),
        };
    }
    if is_instrumental_text(&plain) {
        return Lyrics {
            track_key: track.key.clone(),
            kind: LyricsKind::Instrumental,
            lines: Vec::new(),
            plain,
            source: source.to_string(),
            message: "Instrumental".to_string(),
        };
    }
    Lyrics {
        track_key: track.key.clone(),
        kind: LyricsKind::Plain,
        lines: Vec::new(),
        plain,
        source: source.to_string(),
        message: String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_track() -> Track {
        Track {
            key: "test|track".to_string(),
            title: "Song".to_string(),
            artist: "Artist".to_string(),
            album: "Album".to_string(),
            duration_ms: 200_000,
            cover_png: None,
            accent: None,
            source_app: "test".to_string(),
        }
    }

    #[test]
    fn parses_multiple_timestamps_and_metadata() {
        let lrc = "[ar:Artist]\n[00:01.50][00:03.250]First line\n[00:05]Second line\nplain text\n";
        let lines = parse_lrc(lrc);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].t_ms, 1500);
        assert_eq!(lines[1].t_ms, 3250);
        assert_eq!(lines[2].t_ms, 5000);
        assert_eq!(lines[0].text, "First line");
        assert_eq!(lines[2].text, "Second line");
    }

    #[test]
    fn applies_embedded_offset() {
        let lines = parse_lrc("[offset:500]\n[00:02.00]Shifted\n");
        assert_eq!(lines[0].t_ms, 1500);
    }

    #[test]
    fn parses_enhanced_word_timings() {
        let lines = parse_lrc("[00:10.00]<00:10.00>Hello <00:10.50>world\n");
        assert_eq!(lines[0].t_ms, 10_000);
        assert_eq!(lines[0].text, "Hello world");
        assert_eq!(lines[0].words.len(), 2);
        assert_eq!(lines[0].words[0].t_ms, 10_000);
        assert_eq!(lines[0].words[1].t_ms, 10_500);
        assert_eq!(lines[0].words[1].text, "world");
    }

    #[test]
    fn removes_exact_duplicate_lines() {
        let lines = parse_lrc("[00:01.00]Same\n[00:01.00]Same\n[00:02.00]Other\n");
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[1].text, "Other");
    }

    #[test]
    fn plain_text_has_no_synced_lines() {
        assert!(parse_lrc("Just a line\nAnother line").is_empty());
    }

    #[test]
    fn instrumental_markers_are_detected() {
        assert!(is_instrumental_text("[Instrumental]"));
        assert!(is_instrumental_text("instrumental"));
        assert!(!is_instrumental_text("Play the instrumental break now"));
    }

    #[test]
    fn closest_duration_with_synced_lyrics_wins() {
        let items = vec![
            ApiTrack {
                id: 1,
                track_name: "A".into(),
                artist_name: "X".into(),
                album_name: None,
                duration: Some(200.0),
                synced_lyrics: None,
                plain_lyrics: Some("plain".into()),
                instrumental: Some(false),
            },
            ApiTrack {
                id: 2,
                track_name: "A".into(),
                artist_name: "X".into(),
                album_name: None,
                duration: Some(201.0),
                synced_lyrics: Some("[00:01.00]line".into()),
                plain_lyrics: None,
                instrumental: Some(false),
            },
        ];
        assert_eq!(best_match(&items, 200_500).unwrap().id, 2);
    }

    #[test]
    fn override_is_read_without_network() {
        let directory = tempfile::tempdir().unwrap();
        let service = LyricsService::new(directory.path().to_path_buf());
        let track = test_track();
        let lyrics = service
            .import_override(&track, "[00:01.00]Local line")
            .unwrap();
        assert_eq!(lyrics.kind, LyricsKind::Synced);
        let fetched = service.fetch_for(&track, false);
        assert_eq!(fetched.source, "file");
        assert_eq!(fetched.lines[0].text, "Local line");
    }

    #[test]
    fn missing_override_returns_cache_miss_without_network_when_cached() {
        let directory = tempfile::tempdir().unwrap();
        let service = LyricsService::new(directory.path().to_path_buf());
        let track = test_track();
        let lyrics = none_lyrics(&track, "test");
        cache::write_lyrics(service.cache_dir(), &track.key, Some(&lyrics), true);
        let fetched = service.fetch_for(&track, false);
        assert_eq!(fetched.kind, LyricsKind::None);
    }

    #[test]
    #[ignore]
    fn lrclib_roundtrip() {
        let directory = tempfile::tempdir().unwrap();
        let service = LyricsService::new(directory.path().to_path_buf());
        let results = service.search_free("Midnight City M83").expect("search");
        assert!(!results.is_empty());
        let track = Track {
            key: "test".to_string(),
            title: "Midnight City".to_string(),
            artist: "M83".to_string(),
            album: "Hurry Up, We're Dreaming".to_string(),
            duration_ms: 244_000,
            cover_png: None,
            accent: None,
            source_app: "test".to_string(),
        };
        let lyrics = service.fetch_for(&track, true);
        assert_eq!(lyrics.kind, LyricsKind::Synced);
        assert!(!lyrics.lines.is_empty());
    }
}
