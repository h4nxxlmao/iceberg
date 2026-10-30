use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::lyrics::Lyrics;
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
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
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

pub fn data_url(bytes: &[u8]) -> String {
    format!("data:image/png;base64,{}", STANDARD.encode(bytes))
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
    for dir in [lyrics_dir(cache_dir), art_dir(cache_dir)] {
        collect_files(&dir, &mut files);
    }
    files.sort_by_key(|(modified, _, _)| *modified);
    for (_, path, len) in files {
        if total <= cap {
            break;
        }
        if fs::remove_file(&path).is_ok() {
            total = total.saturating_sub(len);
        }
    }
}

fn collect_files(dir: &Path, out: &mut Vec<(SystemTime, PathBuf, u64)>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if meta.is_file() {
            let modified = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            out.push((modified, path, meta.len()));
        } else if meta.is_dir() {
            collect_files(&path, out);
        }
    }
}

pub fn clear(cache_dir: &Path) -> u64 {
    let mut freed = 0u64;
    for dir in [
        lyrics_dir(cache_dir),
        art_dir(cache_dir),
        override_dir(cache_dir),
    ] {
        freed = freed.saturating_add(dir_size(&dir));
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::create_dir_all(&dir);
    }
    freed
}

pub fn trim_webview_cache(webview_dir: &Path, cap: u64) -> u64 {
    let size = dir_size(webview_dir);
    if size <= cap {
        return 0;
    }
    let mut freed = 0u64;
    let mut targets = Vec::new();
    collect_named_dirs(webview_dir, &mut targets, 0);
    for dir in targets {
        freed = freed.saturating_add(dir_size(&dir));
        let _ = fs::remove_dir_all(&dir);
    }
    freed
}

fn collect_named_dirs(dir: &Path, out: &mut Vec<PathBuf>, depth: usize) {
    if depth > 6 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if !meta.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if matches!(
            name.as_str(),
            "Cache"
                | "Code Cache"
                | "GPUCache"
                | "ShaderCache"
                | "DawnGraphiteCache"
                | "DawnWebGPUCache"
                | "GrShaderCache"
        ) {
            out.push(path);
        } else {
            collect_named_dirs(&path, out, depth + 1);
        }
    }
}

#[allow(dead_code)]
pub fn not_found_age_ok(fetched_at: i64) -> bool {
    now_ms() - fetched_at < Duration::from_secs(24 * 60 * 60).as_millis() as i64
}
