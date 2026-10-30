use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Track {
    pub key: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_ms: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover_png: Option<Vec<u8>>,
    pub accent: Option<String>,
    pub source_app: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Playback {
    pub status: PlaybackStatus,
    pub position_ms: i64,
    pub duration_ms: i64,
    pub received_at_ms: i64,
    pub source_app: String,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackStatus {
    #[default]
    None,
    Playing,
    Paused,
    Stopped,
}

impl Playback {
    /// Position extrapolated to now. The SMTC timeline is only sent when it changes.
    pub fn live_position_ms(&self, now_ms: i64) -> i64 {
        if self.status != PlaybackStatus::Playing {
            return self.position_ms;
        }
        let elapsed = (now_ms - self.received_at_ms).max(0);
        let position = self.position_ms.saturating_add(elapsed);
        if self.duration_ms > 0 {
            position.min(self.duration_ms)
        } else {
            position
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Word {
    pub t_ms: i64,
    pub text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct LyricLine {
    pub t_ms: i64,
    pub text: String,
    pub words: Vec<Word>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LyricsKind {
    Synced,
    Plain,
    Instrumental,
    None,
    Error,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Lyrics {
    pub track_key: String,
    pub kind: LyricsKind,
    pub lines: Vec<LyricLine>,
    pub plain: String,
    pub source: String,
    pub message: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
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
