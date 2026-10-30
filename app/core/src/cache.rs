use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::model::Lyrics;
use crate::util::{cache_name, dir_size, now_ms, replace_file};

pub const CACHE_CAP: u64 = 30 * 1024 * 1024;
pub const NOT_FOUND_TTL_MS: i64 = 24 * 60 * 60 * 1000;

#[derive(Serialize, Deserialize)]
pub struct CacheEntry {
    pub fetched_at: i64,
    pub not_found: bool,
    pub lyrics: Option<Lyrics>,
}

pub fn lyrics_dir(cache_dir: &Path) -> PathBuf {
    cache_dir.join("lyrics")
}

pub fn art_dir(cache_dir: &Path) -> PathBuf {
    cache_dir.join("art")
}

pub fn override_dir(cache_dir: &Path) -> PathBuf {
    cache_dir.join("overrides")
}

pub fn read_lyrics(cache_dir: &Path, key: &str) -> Option<CacheEntry> {
    let path = lyrics_dir(cache_dir).join(format!("{}.json", cache_name(key)));
    let text = fs::read_to_string(path).ok()?;
    let entry: CacheEntry = serde_json::from_str(&text).ok()?;
    if entry.not_found {
        if now_ms() - entry.fetched_at > NOT_FOUND_TTL_MS {
            return None;
        }
        return Some(entry);
    }
    entry.lyrics.as_ref()?;
    Some(entry)
}

pub fn write_lyrics(
    cache_dir: &Path,
    key: &str,
    lyrics: Option<&Lyrics>,
    not_found: bool,
) -> Option<Lyrics> {
    let entry = CacheEntry {
        fetched_at: now_ms(),
        not_found,
        lyrics: lyrics.cloned(),
    };
    let path = lyrics_dir(cache_dir).join(format!("{}.json", cache_name(key)));
    let bytes = serde_json::to_vec(&entry).ok()?;
    replace_file(&path, &bytes).ok()?;
    evict(cache_dir, CACHE_CAP);
    lyrics.cloned()
}

pub fn read_override(cache_dir: &Path, key: &str) -> Option<String> {
    let path = override_dir(cache_dir).join(format!("{}.lrc", cache_name(key)));
    fs::read_to_string(path).ok()
}

pub fn write_override(cache_dir: &Path, key: &str, lrc: &str) -> bool {
    let path = override_dir(cache_dir).join(format!("{}.lrc", cache_name(key)));
    let limited: String = lrc.chars().take(200_000).collect();
    let ok = replace_file(&path, limited.as_bytes()).is_ok();
    if ok {
        evict(cache_dir, CACHE_CAP);
    }
    ok
}

pub fn read_art(cache_dir: &Path, key: &str) -> Option<(Vec<u8>, Option<String>)> {
    let hash = cache_name(key);
    let path = art_dir(cache_dir).join(format!("{hash}.png"));
    let bytes = fs::read(path).ok()?;
    if bytes.is_empty() || bytes.len() > 1_500_000 {
        return None;
    }
    let accent = fs::read_to_string(art_dir(cache_dir).join(format!("{hash}.accent")))
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    Some((bytes, accent))
}

pub fn write_art(cache_dir: &Path, key: &str, bytes: &[u8], accent: Option<&str>) -> bool {
    let hash = cache_name(key);
    let path = art_dir(cache_dir).join(format!("{hash}.png"));
    if replace_file(&path, bytes).is_err() {
        return false;
    }
    if let Some(accent) = accent {
        let _ = replace_file(
            &art_dir(cache_dir).join(format!("{hash}.accent")),
            accent.as_bytes(),
        );
    }
    evict(cache_dir, CACHE_CAP);
    true
}

pub fn cache_size(cache_dir: &Path) -> u64 {
    dir_size(&lyrics_dir(cache_dir))
        .saturating_add(dir_size(&art_dir(cache_dir)))
        .saturating_add(dir_size(&override_dir(cache_dir)))
}

pub fn evict(cache_dir: &Path, cap: u64) {
    let mut total = cache_size(cache_dir);
    if total <= cap {
        return;
    }
    let mut files: Vec<(SystemTime, PathBuf, u64)> = Vec::new();
    for directory in [lyrics_dir(cache_dir), art_dir(cache_dir)] {
        collect_files(&directory, &mut files);
    }
    files.sort_by_key(|(modified, _, _)| *modified);
    for (_, path, length) in files {
        if total <= cap {
            break;
        }
        if fs::remove_file(&path).is_ok() {
            total = total.saturating_sub(length);
        }
    }
}

fn collect_files(directory: &Path, out: &mut Vec<(SystemTime, PathBuf, u64)>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if metadata.is_file() {
            let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            out.push((modified, path, metadata.len()));
        } else if metadata.is_dir() {
            collect_files(&path, out);
        }
    }
}

pub fn clear(cache_dir: &Path) -> u64 {
    let mut freed = 0u64;
    for directory in [
        lyrics_dir(cache_dir),
        art_dir(cache_dir),
        override_dir(cache_dir),
    ] {
        freed = freed.saturating_add(dir_size(&directory));
        let _ = fs::remove_dir_all(&directory);
        let _ = fs::create_dir_all(&directory);
    }
    freed
}

pub fn not_found_is_fresh(fetched_at: i64) -> bool {
    now_ms() - fetched_at < Duration::from_secs(24 * 60 * 60).as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{LyricLine, Lyrics, LyricsKind};

    fn sample_lyrics() -> Lyrics {
        Lyrics {
            track_key: "key".to_string(),
            kind: LyricsKind::Synced,
            lines: vec![LyricLine {
                t_ms: 1000,
                text: "line".to_string(),
                words: Vec::new(),
            }],
            plain: String::new(),
            source: "test".to_string(),
            message: String::new(),
        }
    }

    #[test]
    fn lyrics_cache_roundtrips_and_expires_not_found() {
        let directory = tempfile::tempdir().unwrap();
        let cache = directory.path();
        write_lyrics(cache, "key", Some(&sample_lyrics()), false);
        let entry = read_lyrics(cache, "key").unwrap();
        assert!(!entry.not_found);
        assert_eq!(entry.lyrics.unwrap().lines[0].text, "line");

        let stale = CacheEntry {
            fetched_at: now_ms() - NOT_FOUND_TTL_MS - 1000,
            not_found: true,
            lyrics: None,
        };
        let path = lyrics_dir(cache).join(format!("{}.json", cache_name("missing")));
        replace_file(&path, &serde_json::to_vec(&stale).unwrap()).unwrap();
        assert!(read_lyrics(cache, "missing").is_none());
    }

    #[test]
    fn overrides_and_art_roundtrip() {
        let directory = tempfile::tempdir().unwrap();
        let cache = directory.path();
        assert!(write_override(cache, "key", "[00:01.00]hello"));
        assert_eq!(read_override(cache, "key").unwrap(), "[00:01.00]hello");
        assert!(write_art(cache, "key", &[1, 2, 3], Some("#aabbcc")));
        let (bytes, accent) = read_art(cache, "key").unwrap();
        assert_eq!(bytes, vec![1, 2, 3]);
        assert_eq!(accent.as_deref(), Some("#aabbcc"));
    }

    #[test]
    fn clear_reports_something_and_keeps_directories() {
        let directory = tempfile::tempdir().unwrap();
        let cache = directory.path();
        write_override(cache, "key", "line");
        let freed = clear(cache);
        assert!(freed > 0);
        assert!(override_dir(cache).is_dir());
    }
}
