//! Shared, UI-free core for Iceberg.
//!
//! Everything here is used by both the native widget and the settings app: config and
//! migrations, Windows media sessions, LRCLIB lyrics and caching, font metrics, presets,
//! and the IPC message schema.

pub mod cache;
pub mod config;
pub mod fonts;
pub mod hotkeys;
pub mod ipc;
pub mod lyrics;
pub mod media;
pub mod model;
pub mod paths;
pub mod presets;
pub mod single_instance;
pub mod util;

pub use model::{
    LyricLine, Lyrics, LyricsKind, Playback, PlaybackStatus, SearchResult, Track, Word,
};

pub const APP_NAME: &str = "Iceberg";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
