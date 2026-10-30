use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Mutex, MutexGuard};

use crate::config::{Config, ConfigStore};
use crate::lyrics::{Lyrics, SearchResult};
use crate::timers::TimerHandle;
use crate::util::{clamp_i64, now_ms};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Track {
    pub key: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_ms: i64,
    pub cover: Option<String>,
    pub accent: Option<String>,
    pub source_app: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Playback {
    pub status: String,
    pub position_ms: i64,
    pub duration_ms: i64,
    pub received_at_ms: i64,
    pub source_app: String,
}

impl Default for Playback {
    fn default() -> Self {
        Self {
            status: "none".to_string(),
            position_ms: 0,
            duration_ms: 0,
            received_at_ms: now_ms(),
            source_app: String::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Snapshot {
    pub config: Config,
    pub track: Option<Track>,
    pub playback: Playback,
    pub lyrics: Option<Lyrics>,
    pub effective_offset_ms: i64,
    pub cache_bytes: u64,
    pub app_version: String,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct SavedPosition {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowMemory {
    pub last_monitor: String,
    pub positions: HashMap<String, SavedPosition>,
}

pub struct AppState {
    pub config: Mutex<Config>,
    pub store: ConfigStore,
    pub config_path: PathBuf,
    pub cache_dir: PathBuf,
    pub webview_dir: PathBuf,
    pub window_path: PathBuf,
    pub window_memory: Mutex<WindowMemory>,
    pub track: Mutex<Option<Track>>,
    pub playback: Mutex<Playback>,
    pub lyrics: Mutex<Option<Lyrics>>,
    pub search_results: Mutex<Vec<SearchResult>>,
    pub effective_offset: AtomicI64,
    pub user_hidden: AtomicBool,
    pub peeking: AtomicBool,
    pub paused_hidden: AtomicBool,
    pub fullscreen: AtomicBool,
    pub widget_suspended: AtomicBool,
    pub widget_loaded: AtomicBool,
    pub island_expanded: AtomicBool,
    pub settings_open: AtomicBool,
    pub demo: AtomicBool,
    pub quitting: AtomicBool,
    pub lyrics_generation: AtomicU64,
    pub cache_bytes: AtomicU64,
    pub move_generation: AtomicU64,
    pub smtc_poke: Mutex<Option<Sender<()>>>,
    pub timers: TimerHandle,
}

impl AppState {
    pub fn config(&self) -> Config {
        self.lock_config().clone()
    }

    pub fn lock_config(&self) -> MutexGuard<'_, Config> {
        self.config
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }

    pub fn update_config(&self, mut config: Config) -> Config {
        config.validate();
        {
            let mut guard = self.lock_config();
            *guard = config.clone();
        }
        self.store.queue(config.clone());
        self.refresh_effective_offset();
        config
    }

    pub fn refresh_effective_offset(&self) -> i64 {
        let key = self
            .track
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .as_ref()
            .map(|track| track.key.clone());
        let config = self.lock_config();
        let offset = match key {
            Some(key) => config.effective_offset(&key),
            None => config.lyrics.offset_ms,
        };
        self.effective_offset
            .store(clamp_i64(offset, -20_000, 20_000), Ordering::SeqCst);
        offset
    }

    pub fn effective_offset(&self) -> i64 {
        self.effective_offset.load(Ordering::SeqCst)
    }

    pub fn set_track(&self, track: Option<Track>) {
        if let Ok(mut guard) = self.track.lock() {
            *guard = track;
        }
        self.refresh_effective_offset();
    }

    pub fn current_track(&self) -> Option<Track> {
        self.track
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    pub fn set_playback(&self, playback: Playback) {
        if let Ok(mut guard) = self.playback.lock() {
            *guard = playback;
        }
    }

    pub fn playback(&self) -> Playback {
        self.playback
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    pub fn playback_status(&self) -> String {
        self.playback().status
    }

    pub fn set_lyrics(&self, lyrics: Option<Lyrics>) {
        if let Ok(mut guard) = self.lyrics.lock() {
            *guard = lyrics;
        }
    }

    pub fn current_lyrics(&self) -> Option<Lyrics> {
        self.lyrics
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    pub fn set_search_results(&self, results: Vec<SearchResult>) {
        if let Ok(mut guard) = self.search_results.lock() {
            *guard = results;
        }
    }

    pub fn search_results(&self) -> Vec<SearchResult> {
        self.search_results
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            config: self.config(),
            track: self.current_track(),
            playback: self.playback(),
            lyrics: self.current_lyrics(),
            effective_offset_ms: self.effective_offset(),
            cache_bytes: self.cache_bytes.load(Ordering::Relaxed),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    pub fn set_song_offset(&self, key: &str, offset: i64) -> i64 {
        let mut config = self.config();
        if offset == 0 {
            config.per_song_offsets.remove(key);
        } else {
            config
                .per_song_offsets
                .insert(key.to_string(), clamp_i64(offset, -10_000, 10_000));
        }
        self.update_config(config);
        self.effective_offset()
    }
}
