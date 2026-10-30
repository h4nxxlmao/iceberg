use std::sync::mpsc::{self, Sender};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Manager};
use windows::Foundation::TypedEventHandler;
use windows::Graphics::Imaging::{
    BitmapAlphaMode, BitmapDecoder, BitmapEncoder, BitmapInterpolationMode, BitmapPixelFormat,
    BitmapTransform, ColorManagementMode, ExifOrientationMode,
};
use windows::Media::Control::{
    CurrentSessionChangedEventArgs, GlobalSystemMediaTransportControlsSession,
    GlobalSystemMediaTransportControlsSessionManager as SessionManager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as PlaybackStatus,
    MediaPropertiesChangedEventArgs, PlaybackInfoChangedEventArgs, SessionsChangedEventArgs,
    TimelinePropertiesChangedEventArgs,
};
use windows::Storage::Streams::{
    DataReader, IRandomAccessStream, IRandomAccessStreamWithContentType, InMemoryRandomAccessStream,
};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

use crate::cache;
use crate::config::Config;
use crate::lyrics::{self, emit_position, emit_stopped, emit_track};
use crate::state::{AppState, Playback, Track};
use crate::timers::TimerKind;
use crate::util::{now_ms, track_key};
use crate::window;

const WINRT_TICKS_2020: i64 = 132_000_000_000_000_000;

struct Subscription {
    session: GlobalSystemMediaTransportControlsSession,
    tokens: Vec<i64>,
    source: String,
}

struct Worker {
    app: AppHandle,
    manager: SessionManager,
    tx: Sender<()>,
    session: Option<Subscription>,
    last_meta: Option<(String, String, String, String)>,
    last_key: Option<String>,
    last_playback: Option<(String, i64, i64)>,
    last_status: String,
}

pub fn start(app: AppHandle) {
    thread::Builder::new()
        .name("iceberg-smtc".into())
        .spawn(move || {
            if let Err(error) = run(app.clone()) {
                eprintln!("iceberg: SMTC unavailable: {error}");
                let state = app.state::<AppState>();
                state.set_playback(Playback {
                    status: "none".to_string(),
                    ..Default::default()
                });
            }
        })
        .ok();
}

fn run(app: AppHandle) -> Result<(), String> {
    // SAFETY: this thread is dedicated to SMTC and keeps its COM apartment for the app lifetime.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }

    let manager = SessionManager::RequestAsync()
        .map_err(|error| error.to_string())?
        .join()
        .map_err(|error| error.to_string())?;

    let (tx, rx) = mpsc::channel::<()>();
    {
        let state = app.state::<AppState>();
        if let Ok(mut slot) = state.smtc_poke.lock() {
            *slot = Some(tx.clone());
        };
    };

    let mut manager_tokens: Vec<i64> = Vec::new();
    if let Ok(token) = manager.SessionsChanged(&sessions_handler(tx.clone())) {
        manager_tokens.push(token);
    }
    if let Ok(token) = manager.CurrentSessionChanged(&current_session_handler(tx.clone())) {
        manager_tokens.push(token);
    }

    let mut worker = Worker {
        app,
        manager,
        tx,
        session: None,
        last_meta: None,
        last_key: None,
        last_playback: None,
        last_status: String::new(),
    };

    worker.refresh();

    while rx.recv().is_ok() {
        while rx.recv_timeout(Duration::from_millis(50)).is_ok() {}
        worker.refresh();
    }

    for token in manager_tokens {
        let _ = worker.manager.RemoveSessionsChanged(token);
    }
    Ok(())
}

fn sessions_handler(tx: Sender<()>) -> TypedEventHandler<SessionManager, SessionsChangedEventArgs> {
    TypedEventHandler::new(move |_, _| {
        let _ = tx.send(());
        Ok(())
    })
}

fn current_session_handler(
    tx: Sender<()>,
) -> TypedEventHandler<SessionManager, CurrentSessionChangedEventArgs> {
    TypedEventHandler::new(move |_, _| {
        let _ = tx.send(());
        Ok(())
    })
}

fn media_properties_handler(
    tx: Sender<()>,
) -> TypedEventHandler<GlobalSystemMediaTransportControlsSession, MediaPropertiesChangedEventArgs> {
    TypedEventHandler::new(move |_, _| {
        let _ = tx.send(());
        Ok(())
    })
}

fn playback_info_handler(
    tx: Sender<()>,
) -> TypedEventHandler<GlobalSystemMediaTransportControlsSession, PlaybackInfoChangedEventArgs> {
    TypedEventHandler::new(move |_, _| {
        let _ = tx.send(());
        Ok(())
    })
}

fn timeline_handler(
    tx: Sender<()>,
) -> TypedEventHandler<GlobalSystemMediaTransportControlsSession, TimelinePropertiesChangedEventArgs>
{
    TypedEventHandler::new(move |_, _| {
        let _ = tx.send(());
        Ok(())
    })
}

impl Worker {
    fn refresh(&mut self) {
        let app_handle = self.app.clone();
        let state = app_handle.state::<AppState>();
        let config = state.config();

        let session = match self.manager.GetCurrentSession() {
            Ok(session) => session,
            Err(_) => {
                self.none();
                return;
            }
        };

        let source = session
            .SourceAppUserModelId()
            .map(|value| value.to_string())
            .unwrap_or_default();

        if !app_allowed(&source, &config) {
            self.none();
            return;
        }

        if self
            .session
            .as_ref()
            .map(|current| current.source != source || current.session != session)
            .unwrap_or(true)
        {
            self.subscribe(&session, &source);
        }

        let playback_info = session.GetPlaybackInfo().ok();
        let status = playback_info
            .as_ref()
            .and_then(|info| info.PlaybackStatus().ok())
            .unwrap_or(PlaybackStatus::Closed);
        let status_text = status_text(status);

        if status == PlaybackStatus::Closed || status == PlaybackStatus::Stopped {
            self.none();
            return;
        }

        let properties = session
            .TryGetMediaPropertiesAsync()
            .ok()
            .and_then(|operation| operation.join().ok());
        let Some(properties) = properties else {
            self.none();
            return;
        };

        let title = properties
            .Title()
            .map(|value| value.to_string())
            .unwrap_or_default()
            .trim()
            .to_string();
        let artist = properties
            .Artist()
            .map(|value| value.to_string())
            .unwrap_or_default()
            .trim()
            .to_string();
        let album = properties
            .AlbumTitle()
            .map(|value| value.to_string())
            .unwrap_or_default()
            .trim()
            .to_string();

        if title.is_empty() {
            self.none();
            return;
        }

        let timeline = session.GetTimelineProperties().ok();
        let (mut position_ms, mut duration_ms) = timeline
            .as_ref()
            .map(|timeline| timeline_values(timeline, &status))
            .unwrap_or((0, 0));
        if duration_ms <= 0 {
            if let Some(previous) = self.previous_duration(&title, &artist) {
                duration_ms = previous;
            }
        }
        if duration_ms > 0 {
            position_ms = position_ms.clamp(0, duration_ms);
        } else {
            position_ms = position_ms.max(0);
        }

        let key = track_key(&title, &artist, &album, duration_ms);
        let meta = (key.clone(), title.clone(), artist.clone(), album.clone());
        let changed = self.last_meta.as_ref() != Some(&meta);
        let playback = Playback {
            status: status_text.to_string(),
            position_ms,
            duration_ms,
            received_at_ms: now_ms(),
            source_app: source.clone(),
        };

        if changed {
            let (cover, accent) = match self.cover_for(&properties, &key, &state) {
                Some((data, accent)) => (Some(data), accent),
                None => (None, None),
            };
            let track = Track {
                key: key.clone(),
                title,
                artist,
                album,
                duration_ms,
                cover,
                accent,
                source_app: source,
            };
            state.set_track(Some(track.clone()));
            state.set_playback(playback.clone());
            state
                .paused_hidden
                .store(false, std::sync::atomic::Ordering::SeqCst);
            emit_track(&self.app, Some(&track), &playback);
            crate::tray::set_playing(&self.app, playback.status == "playing");
            lyrics::start_fetch(self.app.clone(), track, false);
            self.last_key = Some(key);
            self.last_meta = Some(meta);
            self.last_status = playback.status.clone();
            self.last_playback = Some((
                playback.status.clone(),
                playback.position_ms,
                playback.duration_ms,
            ));
        } else {
            state.set_playback(playback.clone());
            let signature = (
                playback.status.clone(),
                playback.position_ms,
                playback.duration_ms,
            );
            if self.last_playback.as_ref() != Some(&signature) {
                emit_position(&self.app, &playback);
                self.last_playback = Some(signature);
            }
        }

        if self.last_status != playback.status {
            self.last_status = playback.status.clone();
            crate::tray::set_playing(&self.app, playback.status == "playing");
        }

        self.apply_timing(&config, &playback);
    }

    fn previous_duration(&self, title: &str, artist: &str) -> Option<i64> {
        if let Some((_, prev_title, prev_artist, _)) = &self.last_meta {
            if prev_title == title && prev_artist == artist {
                let state = self.app.state::<AppState>();
                let playback = state.playback();
                return Some(playback.duration_ms).filter(|value| *value > 0);
            }
        }
        None
    }

    fn cover_for(
        &self,
        properties: &windows::Media::Control::GlobalSystemMediaTransportControlsSessionMediaProperties,
        key: &str,
        state: &AppState,
    ) -> Option<(String, Option<String>)> {
        if let Some((bytes, accent)) = cache::read_art(&state.cache_dir, key) {
            return Some((cache::data_url(&bytes), accent));
        }
        let thumbnail = properties.Thumbnail().ok()?;
        let stream = thumbnail.OpenReadAsync().ok()?.join().ok()?;
        let (bytes, accent) = decode_thumbnail(&stream).ok()?;
        if bytes.len() > 1_500_000 {
            return None;
        }
        cache::write_art(&state.cache_dir, key, &bytes, accent.as_deref());
        Some((cache::data_url(&bytes), accent))
    }

    fn subscribe(&mut self, session: &GlobalSystemMediaTransportControlsSession, source: &str) {
        if let Some(old) = self.session.take() {
            if let Some(token) = old.tokens.first() {
                let _ = old.session.RemoveMediaPropertiesChanged(*token);
            }
            if let Some(token) = old.tokens.get(1) {
                let _ = old.session.RemovePlaybackInfoChanged(*token);
            }
            if let Some(token) = old.tokens.get(2) {
                let _ = old.session.RemoveTimelinePropertiesChanged(*token);
            }
        }

        let mut tokens = Vec::new();
        if let Ok(token) =
            session.MediaPropertiesChanged(&media_properties_handler(self.tx.clone()))
        {
            tokens.push(token);
        }
        if let Ok(token) = session.PlaybackInfoChanged(&playback_info_handler(self.tx.clone())) {
            tokens.push(token);
        }
        if let Ok(token) = session.TimelinePropertiesChanged(&timeline_handler(self.tx.clone())) {
            tokens.push(token);
        }
        self.session = Some(Subscription {
            session: session.clone(),
            tokens,
            source: source.to_string(),
        });
    }

    fn apply_timing(&self, config: &Config, playback: &Playback) {
        let state = self.app.state::<AppState>();
        match playback.status.as_str() {
            "playing" => {
                state
                    .paused_hidden
                    .store(false, std::sync::atomic::Ordering::SeqCst);
                state.timers.cancel(TimerKind::IdleDestroy);
                state.timers.cancel(TimerKind::PausedHide);
                window::update_visibility(&self.app);
            }
            "paused" => {
                let idle = Duration::from_secs(config.behavior.idle_destroy_minutes as u64 * 60);
                state.timers.set(TimerKind::IdleDestroy, idle);
                if config.behavior.auto_hide_paused_seconds == 0 {
                    state
                        .paused_hidden
                        .store(true, std::sync::atomic::Ordering::SeqCst);
                    window::update_visibility(&self.app);
                } else {
                    state.timers.set(
                        TimerKind::PausedHide,
                        Duration::from_secs(config.behavior.auto_hide_paused_seconds as u64),
                    );
                    window::update_visibility(&self.app);
                }
            }
            _ => {
                state
                    .paused_hidden
                    .store(true, std::sync::atomic::Ordering::SeqCst);
                state.timers.cancel(TimerKind::PausedHide);
                state.timers.set(
                    TimerKind::IdleDestroy,
                    Duration::from_secs(config.behavior.idle_destroy_minutes as u64 * 60),
                );
                window::update_visibility(&self.app);
            }
        }
    }

    fn none(&mut self) {
        let app_handle = self.app.clone();
        let state = app_handle.state::<AppState>();
        let had_track = state.current_track().is_some() || self.last_key.is_some();
        self.last_meta = None;
        self.last_key = None;
        self.last_playback = None;
        self.last_status = "stopped".to_string();
        if had_track {
            state.set_track(None);
            state.set_lyrics(None);
            state.set_playback(Playback {
                status: "stopped".to_string(),
                ..Default::default()
            });
            emit_stopped(&self.app);
            crate::tray::set_playing(&self.app, false);
        }
        state
            .paused_hidden
            .store(true, std::sync::atomic::Ordering::SeqCst);
        state.timers.cancel(TimerKind::PausedHide);
        let config = state.config();
        state.timers.set(
            TimerKind::IdleDestroy,
            Duration::from_secs(config.behavior.idle_destroy_minutes as u64 * 60),
        );
        window::update_visibility(&self.app);
    }
}

fn status_text(status: PlaybackStatus) -> &'static str {
    if status == PlaybackStatus::Playing {
        "playing"
    } else if status == PlaybackStatus::Paused
        || status == PlaybackStatus::Opened
        || status == PlaybackStatus::Changing
    {
        "paused"
    } else {
        "stopped"
    }
}

fn timeline_values(
    timeline: &windows::Media::Control::GlobalSystemMediaTransportControlsSessionTimelineProperties,
    status: &PlaybackStatus,
) -> (i64, i64) {
    let start = timeline
        .StartTime()
        .map(|value| value.Duration)
        .unwrap_or(0);
    let end = timeline.EndTime().map(|value| value.Duration).unwrap_or(0);
    let raw_position = timeline.Position().map(|value| value.Duration).unwrap_or(0);
    let duration = (end - start).max(0) / 10_000;
    let mut position = (raw_position - start).max(0) / 10_000;

    if *status == PlaybackStatus::Playing {
        if let Ok(updated) = timeline.LastUpdatedTime() {
            let updated = updated.UniversalTime;
            if updated > WINRT_TICKS_2020 {
                let now = winrt_now_ticks();
                let elapsed = (now - updated).max(0) / 10_000;
                position += elapsed;
            }
        }
    }
    (position, duration)
}

fn winrt_now_ticks() -> i64 {
    let unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    (unix.as_secs() as i64 + 11_644_473_600) * 10_000_000 + (unix.subsec_nanos() as i64 / 100)
}

fn app_allowed(source: &str, config: &Config) -> bool {
    let source = source.to_lowercase();
    if !config.behavior.ignored_apps.is_empty()
        && config
            .behavior
            .ignored_apps
            .iter()
            .any(|app| source.contains(&app.to_lowercase()))
    {
        return false;
    }
    if config.behavior.preferred_apps.is_empty() {
        return true;
    }
    config
        .behavior
        .preferred_apps
        .iter()
        .any(|app| source.contains(&app.to_lowercase()))
}

fn decode_thumbnail(
    stream: &IRandomAccessStreamWithContentType,
) -> Result<(Vec<u8>, Option<String>), String> {
    let accent_stream: Option<IRandomAccessStream> = stream.CloneStream().ok();

    let decoder = BitmapDecoder::CreateAsync(stream)
        .map_err(|error| error.to_string())?
        .join()
        .map_err(|error| error.to_string())?;

    let transform = BitmapTransform::new().map_err(|error| error.to_string())?;
    let _ = transform.SetScaledWidth(160);
    let _ = transform.SetScaledHeight(160);
    let _ = transform.SetInterpolationMode(BitmapInterpolationMode::Fant);
    let bitmap = decoder
        .GetSoftwareBitmapTransformedAsync(
            BitmapPixelFormat::Bgra8,
            BitmapAlphaMode::Premultiplied,
            &transform,
            ExifOrientationMode::IgnoreExifOrientation,
            ColorManagementMode::DoNotColorManage,
        )
        .map_err(|error| error.to_string())?
        .join()
        .map_err(|error| error.to_string())?;

    let out = InMemoryRandomAccessStream::new().map_err(|error| error.to_string())?;
    let encoder = BitmapEncoder::CreateAsync(
        BitmapEncoder::PngEncoderId().map_err(|error| error.to_string())?,
        &out,
    )
    .map_err(|error| error.to_string())?
    .join()
    .map_err(|error| error.to_string())?;
    encoder
        .SetSoftwareBitmap(&bitmap)
        .map_err(|error| error.to_string())?;
    encoder
        .FlushAsync()
        .map_err(|error| error.to_string())?
        .join()
        .map_err(|error| error.to_string())?;

    let size = out.Size().map_err(|error| error.to_string())?;
    if size == 0 || size > 1_500_000 {
        return Err("bad thumbnail size".to_string());
    }
    let input = out.GetInputStreamAt(0).map_err(|error| error.to_string())?;
    let reader = DataReader::CreateDataReader(&input).map_err(|error| error.to_string())?;
    reader
        .LoadAsync(size as u32)
        .map_err(|error| error.to_string())?
        .join()
        .map_err(|error| error.to_string())?;
    let mut bytes = vec![0u8; size as usize];
    reader
        .ReadBytes(&mut bytes)
        .map_err(|error| error.to_string())?;

    let accent = accent_stream
        .and_then(|stream| accent_from_stream(&stream).ok())
        .flatten();

    Ok((bytes, accent))
}

fn accent_from_stream(stream: &IRandomAccessStream) -> Result<Option<String>, String> {
    let decoder = BitmapDecoder::CreateAsync(stream)
        .map_err(|error| error.to_string())?
        .join()
        .map_err(|error| error.to_string())?;
    let transform = BitmapTransform::new().map_err(|error| error.to_string())?;
    let _ = transform.SetScaledWidth(10);
    let _ = transform.SetScaledHeight(10);
    let _ = transform.SetInterpolationMode(BitmapInterpolationMode::Fant);
    let pixels = decoder
        .GetPixelDataTransformedAsync(
            BitmapPixelFormat::Bgra8,
            BitmapAlphaMode::Premultiplied,
            &transform,
            ExifOrientationMode::IgnoreExifOrientation,
            ColorManagementMode::DoNotColorManage,
        )
        .map_err(|error| error.to_string())?
        .join()
        .map_err(|error| error.to_string())?
        .DetachPixelData()
        .map_err(|error| error.to_string())?;
    Ok(dominant_color(pixels.as_ref()))
}

fn dominant_color(pixels: &[u8]) -> Option<String> {
    let mut weight_sum = 0f64;
    let mut red = 0f64;
    let mut green = 0f64;
    let mut blue = 0f64;
    for pixel in pixels.chunks_exact(4) {
        let b = pixel[0] as f64 / 255.0;
        let g = pixel[1] as f64 / 255.0;
        let r = pixel[2] as f64 / 255.0;
        let a = pixel[3] as f64 / 255.0;
        if a < 0.4 {
            continue;
        }
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let lightness = (max + min) / 2.0;
        if !(0.12..=0.9).contains(&lightness) {
            continue;
        }
        let saturation = if max <= 0.001 { 0.0 } else { (max - min) / max };
        let weight = (saturation * 2.2 + 0.12) * (1.0 - (lightness - 0.52).abs());
        if weight <= 0.0 {
            continue;
        }
        weight_sum += weight;
        red += r * weight;
        green += g * weight;
        blue += b * weight;
    }
    if weight_sum <= 0.0 {
        return None;
    }
    let r = (red / weight_sum * 255.0).round().clamp(0.0, 255.0) as u8;
    let g = (green / weight_sum * 255.0).round().clamp(0.0, 255.0) as u8;
    let b = (blue / weight_sum * 255.0).round().clamp(0.0, 255.0) as u8;
    Some(format!("#{r:02x}{g:02x}{b:02x}"))
}
