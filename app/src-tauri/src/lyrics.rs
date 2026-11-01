use serde::{Deserialize, Serialize};
use std::io::Read;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};

use crate::cache;
use crate::state::{AppState, Playback, Track};
use crate::util::{clean_artist, clean_title, now_ms};

const USER_AGENT: &str = "Iceberg/0.2.0 (https://github.com/h4nxxlmao/iceberg)";
const MAX_BODY: u64 = 2 * 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LyricsKind {
    Synced,
    Plain,
    Instrumental,
    None,
    Error,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LyricLine {
    pub t: i64,
    pub text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Lyrics {
    pub track_key: String,
    pub kind: LyricsKind,
    pub lines: Vec<LyricLine>,
    pub plain: String,
    pub source: String,
    pub message: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchResult {
    pub id: u64,
    pub track_name: String,
    pub artist_name: String,
    pub album_name: String,
    pub duration: f64,
    pub instrumental: bool,
    pub has_synced: bool,
    pub has_plain: bool,
}

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

    fn to_search_result(&self) -> SearchResult {
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

#[derive(Serialize, Clone)]
struct LyricsPayload {
    lyrics: Lyrics,
    effective_offset_ms: i64,
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(5))
        .user_agent(USER_AGENT)
        .build()
}

fn read_body(response: ureq::Response) -> Result<String, String> {
    let mut body = String::new();
    response
        .into_reader()
        .take(MAX_BODY)
        .read_to_string(&mut body)
        .map_err(|error| error.to_string())?;
    Ok(body)
}

fn http_json<T: for<'de> Deserialize<'de>>(
    url: &str,
    query: &[(String, String)],
) -> Result<Option<T>, String> {
    let mut last_error = String::new();
    for attempt in 0..2 {
        let mut request = agent().get(url);
        for (key, value) in query {
            request = request.query(key, value);
        }
        match request.call() {
            Ok(response) => {
                let body = read_body(response)?;
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

fn api_get_by_id(id: u64) -> Result<Option<ApiTrack>, String> {
    let url = format!("https://lrclib.net/api/get/{id}");
    http_json(&url, &[])
}

fn api_get(track: &Track) -> Result<Option<ApiTrack>, String> {
    let title = clean_title(&track.title);
    let artist = clean_artist(&track.artist);
    let album = track.album.trim().to_string();
    let mut query = vec![
        ("track_name".to_string(), title),
        ("artist_name".to_string(), artist),
    ];
    if !album.is_empty() {
        query.push(("album_name".to_string(), album));
    }
    if track.duration_ms > 0 {
        query.push((
            "duration".to_string(),
            (track.duration_ms / 1000).to_string(),
        ));
    }
    http_json("https://lrclib.net/api/get", &query)
}

fn api_search(track: &Track) -> Result<Vec<ApiTrack>, String> {
    let query = vec![
        ("track_name".to_string(), clean_title(&track.title)),
        ("artist_name".to_string(), clean_artist(&track.artist)),
    ];
    Ok(http_json::<Vec<ApiTrack>>("https://lrclib.net/api/search", &query)?.unwrap_or_default())
}

pub fn search_free(query: &str) -> Result<Vec<SearchResult>, String> {
    let clean = query.trim();
    if clean.is_empty() {
        return Ok(Vec::new());
    }
    let params = vec![("q".to_string(), clean.chars().take(200).collect())];
    let items =
        http_json::<Vec<ApiTrack>>("https://lrclib.net/api/search", &params)?.unwrap_or_default();
    Ok(items
        .iter()
        .take(20)
        .map(ApiTrack::to_search_result)
        .collect())
}

fn best_match(items: &[ApiTrack], duration_ms: i64) -> Option<&ApiTrack> {
    if items.is_empty() {
        return None;
    }
    let target = duration_ms as f64 / 1000.0;
    let mut best: Option<(&ApiTrack, f64, bool)> = None;
    for item in items {
        let diff = if item.duration.is_some() {
            (item.duration_seconds() - target).abs()
        } else {
            f64::INFINITY
        };
        let within = duration_ms <= 0 || diff <= 2.0;
        let synced = item.has_synced();
        let score = (within, synced, -diff);
        match best {
            None => best = Some((item, diff, within)),
            Some((current, current_diff, current_within)) => {
                let current_score = (current_within, current.has_synced(), -current_diff);
                if score > current_score {
                    best = Some((item, diff, within));
                }
            }
        }
    }
    best.map(|(item, _, _)| item)
}

fn fetch_network(track: &Track) -> Result<Option<ApiTrack>, String> {
    if let Some(item) = api_get(track)? {
        if item.has_synced() || item.has_plain() || item.is_instrumental() {
            return Ok(Some(item));
        }
    }
    let results = api_search(track)?;
    let Some(best) = best_match(&results, track.duration_ms) else {
        return Ok(None);
    };
    if best.has_synced() || best.has_plain() || best.is_instrumental() {
        return Ok(Some(best.clone()));
    }
    if best.id > 0 {
        if let Some(detail) = api_get_by_id(best.id)? {
            return Ok(Some(detail));
        }
    }
    Ok(None)
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
        let content = rest.trim().to_string();
        for stamp in stamps {
            let adjusted = (stamp - embedded_offset).max(0);
            lines.push(LyricLine {
                t: adjusted,
                text: content.clone(),
            });
        }
        if lines.len() > 5000 {
            break;
        }
    }

    lines.sort_by_key(|line| line.t);
    lines.dedup_by(|a, b| a.t == b.t && a.text == b.text);
    lines
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
    lower.trim() == "instrumental"
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

    Lyrics {
        track_key: track.key.clone(),
        kind: LyricsKind::None,
        lines: Vec::new(),
        plain: String::new(),
        source: "lrclib".to_string(),
        message: "No lyrics found".to_string(),
    }
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

pub fn emit_lyrics(app: &AppHandle, lyrics: &Lyrics, offset: i64) {
    let _ = app.emit(
        "lyrics-loaded",
        LyricsPayload {
            lyrics: lyrics.clone(),
            effective_offset_ms: offset,
        },
    );
}

pub fn fetch_for(state: &AppState, track: &Track, force: bool) -> Lyrics {
    if !force {
        if let Some(text) = cache::read_override(&state.cache_dir, &track.key) {
            return lyrics_from_manual(track, &text, "file");
        }
        if let Some(entry) = cache::read_lyrics(&state.cache_dir, &track.key) {
            if entry.not_found {
                return Lyrics {
                    track_key: track.key.clone(),
                    kind: LyricsKind::None,
                    lines: Vec::new(),
                    plain: String::new(),
                    source: "cache".to_string(),
                    message: "No lyrics found".to_string(),
                };
            }
            if let Some(mut lyrics) = entry.lyrics {
                lyrics.source = "cache".to_string();
                return lyrics;
            }
        }
    }

    match fetch_network(track) {
        Ok(Some(api)) => {
            let lyrics = lyrics_from_api(track, &api);
            cache::write_lyrics(
                &state.cache_dir,
                &track.key,
                Some(&lyrics),
                lyrics.kind == LyricsKind::None,
            );
            lyrics
        }
        Ok(None) => {
            let lyrics = Lyrics {
                track_key: track.key.clone(),
                kind: LyricsKind::None,
                lines: Vec::new(),
                plain: String::new(),
                source: "lrclib".to_string(),
                message: "No lyrics found".to_string(),
            };
            cache::write_lyrics(&state.cache_dir, &track.key, None, true);
            lyrics
        }
        Err(error) => Lyrics {
            track_key: track.key.clone(),
            kind: LyricsKind::Error,
            lines: Vec::new(),
            plain: String::new(),
            source: "lrclib".to_string(),
            message: if error.contains("Network") || error.contains("Network error") {
                "Offline or LRCLIB unreachable".to_string()
            } else {
                "Lyrics unavailable".to_string()
            },
        },
    }
}

pub fn start_fetch(app: AppHandle, track: Track, force: bool) {
    let state = app.state::<AppState>();
    let generation = state.lyrics_generation.fetch_add(1, Ordering::SeqCst) + 1;
    let existing = state.current_lyrics();
    if existing.as_ref().map(|lyrics| lyrics.track_key.as_str()) != Some(track.key.as_str()) {
        state.set_lyrics(None);
    }

    thread::Builder::new()
        .name("iceberg-lyrics".into())
        .spawn(move || {
            let state = app.state::<AppState>();
            let lyrics = fetch_for(&state, &track, force);
            if state.lyrics_generation.load(Ordering::SeqCst) != generation {
                return;
            }
            let current = state.current_track();
            if current.as_ref().map(|item| item.key.as_str()) != Some(track.key.as_str()) {
                return;
            }
            state.set_lyrics(Some(lyrics.clone()));
            state
                .cache_bytes
                .store(cache::cache_size(&state.cache_dir), Ordering::Relaxed);
            emit_lyrics(&app, &lyrics, state.effective_offset());
        })
        .ok();
}

pub fn apply_manual(state: &AppState, app: &AppHandle, text: &str) -> Result<Lyrics, String> {
    let track = state.current_track().ok_or("Nothing is playing")?;
    let lyrics = lyrics_from_manual(&track, text, "file");
    if lyrics.kind == LyricsKind::None {
        return Err("That file has no lyrics".to_string());
    }
    cache::write_override(&state.cache_dir, &track.key, text);
    state.lyrics_generation.fetch_add(1, Ordering::SeqCst);
    state.set_lyrics(Some(lyrics.clone()));
    state
        .cache_bytes
        .store(cache::cache_size(&state.cache_dir), Ordering::Relaxed);
    emit_lyrics(app, &lyrics, state.effective_offset());
    Ok(lyrics)
}

pub fn apply_search_choice(state: &AppState, app: &AppHandle, id: u64) -> Result<(), String> {
    let track = state.current_track().ok_or("Nothing is playing")?;
    let known = state.search_results().iter().any(|item| item.id == id);
    if !known {
        return Err("That result is no longer available".to_string());
    }
    let api = api_get_by_id(id)?.ok_or("LRCLIB has no record for that result")?;
    let lyrics = lyrics_from_api(&track, &api);
    if lyrics.kind == LyricsKind::None {
        return Err("That result has no lyrics".to_string());
    }
    let raw = api
        .synced_lyrics
        .as_deref()
        .filter(|text| !text.trim().is_empty())
        .or(api.plain_lyrics.as_deref())
        .unwrap_or("");
    cache::write_override(&state.cache_dir, &track.key, raw);
    state.lyrics_generation.fetch_add(1, Ordering::SeqCst);
    state.set_lyrics(Some(lyrics.clone()));
    state
        .cache_bytes
        .store(cache::cache_size(&state.cache_dir), Ordering::Relaxed);
    emit_lyrics(app, &lyrics, state.effective_offset());
    Ok(())
}

pub fn set_offset(state: &AppState, app: &AppHandle, delta: i64) -> i64 {
    let Some(track) = state.current_track() else {
        return state.effective_offset();
    };
    let config = state.config();
    let current = config.per_song_offset(&track.key);
    let offset = state.set_song_offset(&track.key, current + delta);
    let _ = app.emit(
        "offset-indicator",
        serde_json::json!({ "offset_ms": offset, "per_song": true }),
    );
    let _ = app.emit("position-sync", state.playback());
    offset
}

pub fn reset_offset(state: &AppState, app: &AppHandle) -> i64 {
    let Some(track) = state.current_track() else {
        return state.effective_offset();
    };
    let offset = state.set_song_offset(&track.key, 0);
    let _ = app.emit(
        "offset-indicator",
        serde_json::json!({ "offset_ms": offset, "per_song": false }),
    );
    let _ = app.emit("position-sync", state.playback());
    offset
}

#[derive(Serialize, Clone)]
struct ConfigPayload<'a> {
    config: &'a crate::config::Config,
    effective_offset_ms: i64,
}

pub fn emit_config(app: &AppHandle, config: &crate::config::Config, effective_offset_ms: i64) {
    let _ = app.emit(
        "config-changed",
        ConfigPayload {
            config,
            effective_offset_ms,
        },
    );
}

pub fn emit_track(app: &AppHandle, track: Option<&Track>, playback: &Playback) {
    let _ = app.emit(
        "track-changed",
        serde_json::json!({ "track": track, "playback": playback }),
    );
}

pub fn emit_position(app: &AppHandle, playback: &Playback) {
    let _ = app.emit("position-sync", playback);
}

pub fn emit_stopped(app: &AppHandle) {
    let playback = Playback {
        status: "stopped".to_string(),
        position_ms: 0,
        duration_ms: 0,
        received_at_ms: now_ms(),
        source_app: String::new(),
    };
    let _ = app.emit(
        "track-changed",
        serde_json::json!({ "track": null, "playback": playback }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_multiple_timestamps_and_metadata() {
        let lrc = "[ar:Artist]\n[00:01.50][00:03.250]First line\n[00:05]Second line\nplain text\n";
        let lines = parse_lrc(lrc);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].t, 1500);
        assert_eq!(lines[1].t, 3250);
        assert_eq!(lines[2].t, 5000);
        assert_eq!(lines[0].text, "First line");
        assert_eq!(lines[2].text, "Second line");
    }

    #[test]
    fn applies_embedded_offset() {
        let lrc = "[offset:500]\n[00:02.00]Shifted\n";
        let lines = parse_lrc(lrc);
        assert_eq!(lines[0].t, 1500);
    }

    #[test]
    fn plain_text_has_no_synced_lines() {
        assert!(parse_lrc("Just a line\nAnother line").is_empty());
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
        let best = best_match(&items, 200_500).expect("match");
        assert_eq!(best.id, 2);
    }

    #[test]
    fn instrumental_plain_text_is_detected() {
        let track = Track {
            key: "k".into(),
            title: "Song".into(),
            artist: "Artist".into(),
            album: String::new(),
            duration_ms: 1000,
            cover: None,
            accent: None,
            source_app: String::new(),
        };
        let api = ApiTrack {
            id: 3,
            track_name: "Song".into(),
            artist_name: "Artist".into(),
            album_name: None,
            duration: Some(1.0),
            synced_lyrics: None,
            plain_lyrics: Some("[Instrumental]".into()),
            instrumental: Some(false),
        };
        assert_eq!(lyrics_from_api(&track, &api).kind, LyricsKind::Instrumental);
    }

    #[test]
    fn instrumental_markers_are_not_confused_with_lyrics() {
        assert!(is_instrumental_text("[Instrumental]"));
        assert!(is_instrumental_text("instrumental"));
        assert!(!is_instrumental_text("Play the instrumental break now"));
    }

    #[test]
    #[ignore]
    fn lrclib_roundtrip() {
        let results = search_free("Midnight City M83").expect("search");
        assert!(!results.is_empty());

        let track = Track {
            key: "test".into(),
            title: "Midnight City (Official Video)".into(),
            artist: "M83 - Topic".into(),
            album: "Hurry Up, We're Dreaming".into(),
            duration_ms: 244_000,
            cover: None,
            accent: None,
            source_app: "test".into(),
        };
        let found = fetch_network(&track).expect("fetch").expect("track");
        assert!(found.has_synced());
        assert!(!lyrics_from_api(&track, &found).lines.is_empty());
    }
}
