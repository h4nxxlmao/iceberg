use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use windows::Foundation::TypedEventHandler;
use windows::Graphics::Imaging::{
    BitmapAlphaMode, BitmapDecoder, BitmapEncoder, BitmapInterpolationMode, BitmapPixelFormat,
    BitmapTransform, ColorManagementMode, ExifOrientationMode,
};
use windows::Media::Control::{
    CurrentSessionChangedEventArgs, GlobalSystemMediaTransportControlsSession,
    GlobalSystemMediaTransportControlsSessionManager as SessionManager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as PlaybackStatusApi,
    MediaPropertiesChangedEventArgs, PlaybackInfoChangedEventArgs, SessionsChangedEventArgs,
    TimelinePropertiesChangedEventArgs,
};
use windows::Storage::Streams::{
    DataReader, IRandomAccessStream, IRandomAccessStreamWithContentType, InMemoryRandomAccessStream,
};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

use crate::cache;
use crate::config::Config;
use crate::model::{Playback, PlaybackStatus, Track};
use crate::util::{now_ms, track_key};

const WINRT_TICKS_2020: i64 = 132_000_000_000_000_000;

#[derive(Clone, Debug)]
pub enum MediaEvent {
    Track { track: Track, playback: Playback },
    Playback(Playback),
    Stopped,
    NoSession,
    Error(String),
}

pub struct MediaWatcher {
    stop: Arc<AtomicBool>,
    force_track: Arc<AtomicBool>,
    wake: Sender<()>,
    thread: Option<thread::JoinHandle<()>>,
}

impl MediaWatcher {
    pub fn start(
        cache_dir: PathBuf,
        config: Arc<Mutex<Config>>,
        sender: impl Fn(MediaEvent) + Send + 'static,
    ) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let force_track = Arc::new(AtomicBool::new(false));
        let (wake, receiver) = mpsc::channel::<()>();
        let stop_for_thread = stop.clone();
        let force_for_thread = force_track.clone();
        let wake_for_thread = wake.clone();
        let thread = thread::Builder::new()
            .name("iceberg-media".into())
            .spawn(move || {
                // SAFETY: this thread is dedicated to SMTC and keeps its COM apartment for
                // the lifetime of the watcher.
                unsafe {
                    let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
                }
                if let Err(error) = run(
                    cache_dir,
                    config,
                    receiver,
                    stop_for_thread,
                    force_for_thread,
                    wake_for_thread,
                    sender,
                ) {
                    let _ = error;
                }
            })
            .ok();
        Self {
            stop,
            force_track,
            wake,
            thread,
        }
    }

    /// Ask the media worker to emit the current track even when the metadata has not changed.
    /// This recovers the native state if a track event was missed or a preview replaced it.
    pub fn request_track(&self) {
        self.force_track.store(true, Ordering::SeqCst);
        let _ = self.wake.send(());
    }

    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = self.wake.send(());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for MediaWatcher {
    fn drop(&mut self) {
        self.stop();
    }
}

struct Subscription {
    session: GlobalSystemMediaTransportControlsSession,
    tokens: Vec<i64>,
    source: String,
}

struct Worker {
    manager: SessionManager,
    tx: Sender<()>,
    force_track: Arc<AtomicBool>,
    config: Arc<Mutex<Config>>,
    cache_dir: PathBuf,
    sender: Box<dyn Fn(MediaEvent) + Send>,
    session: Option<Subscription>,
    last_meta: Option<(String, String, String, String)>,
    last_key: Option<String>,
    last_playback: Option<(PlaybackStatus, i64, i64)>,
}

fn run(
    cache_dir: PathBuf,
    config: Arc<Mutex<Config>>,
    receiver: Receiver<()>,
    stop: Arc<AtomicBool>,
    force_track: Arc<AtomicBool>,
    wake: Sender<()>,
    sender: impl Fn(MediaEvent) + Send + 'static,
) -> Result<(), String> {
    let manager = match SessionManager::RequestAsync()
        .map_err(|error| error.to_string())
        .and_then(|operation| operation.join().map_err(|error| error.to_string()))
    {
        Ok(manager) => manager,
        Err(error) => {
            sender(MediaEvent::Error(error));
            return Ok(());
        }
    };

    let mut manager_tokens: Vec<i64> = Vec::new();
    if let Ok(token) = manager.SessionsChanged(&sessions_handler(wake.clone())) {
        manager_tokens.push(token);
    }
    if let Ok(token) = manager.CurrentSessionChanged(&current_session_handler(wake.clone())) {
        manager_tokens.push(token);
    }

    let mut worker = Worker {
        manager,
        tx: wake.clone(),
        force_track,
        config,
        cache_dir,
        sender: Box::new(sender),
        session: None,
        last_meta: None,
        last_key: None,
        last_playback: None,
    };

    worker.refresh();

    while receiver.recv().is_ok() {
        if stop.load(Ordering::SeqCst) {
            break;
        }
        // Coalesce the burst of events that often follows a track change.
        while receiver.try_recv().is_ok() {}
        worker.refresh();
    }

    if let Some(session) = worker.session.take() {
        remove_subscription(&session);
    }
    for token in manager_tokens {
        let _ = worker.manager.RemoveSessionsChanged(token);
    }
    Ok(())
}

fn remove_subscription(subscription: &Subscription) {
    if let Some(token) = subscription.tokens.first() {
        let _ = subscription.session.RemoveMediaPropertiesChanged(*token);
    }
    if let Some(token) = subscription.tokens.get(1) {
        let _ = subscription.session.RemovePlaybackInfoChanged(*token);
    }
    if let Some(token) = subscription.tokens.get(2) {
        let _ = subscription.session.RemoveTimelinePropertiesChanged(*token);
    }
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
    fn send(&self, event: MediaEvent) {
        (self.sender)(event);
    }

    fn refresh(&mut self) {
        let force_track = self.force_track.swap(false, Ordering::SeqCst);
        let config = self
            .config
            .lock()
            .map(|config| config.clone())
            .unwrap_or_default();

        let session = match self.manager.GetCurrentSession() {
            Ok(session) => session,
            Err(_) => {
                self.none(MediaEvent::NoSession);
                return;
            }
        };

        let source = session
            .SourceAppUserModelId()
            .map(|value| value.to_string())
            .unwrap_or_default();

        if !app_allowed(&source, &config) {
            self.none(MediaEvent::NoSession);
            return;
        }

        let session_changed = self
            .session
            .as_ref()
            .map(|current| current.source != source || current.session != session)
            .unwrap_or(true);
        if session_changed {
            self.subscribe(&session, &source);
        }

        let playback_info = session.GetPlaybackInfo().ok();
        let api_status = playback_info
            .as_ref()
            .and_then(|info| info.PlaybackStatus().ok())
            .unwrap_or(PlaybackStatusApi::Closed);
        let status = status_value(api_status);

        if status == PlaybackStatus::Stopped {
            self.none(MediaEvent::Stopped);
            return;
        }

        let properties = session
            .TryGetMediaPropertiesAsync()
            .ok()
            .and_then(|operation| operation.join().ok());
        let Some(properties) = properties else {
            self.none(MediaEvent::NoSession);
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
            self.none(MediaEvent::NoSession);
            return;
        }

        let timeline = session.GetTimelineProperties().ok();
        let (mut position_ms, mut duration_ms) = timeline
            .as_ref()
            .map(|timeline| timeline_values(timeline, &api_status))
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
            status,
            position_ms,
            duration_ms,
            received_at_ms: now_ms(),
            source_app: source.clone(),
        };

        if changed || force_track {
            let (cover_png, accent) = match self.cover_for(&properties, &key) {
                Some((bytes, accent)) => (Some(bytes), accent),
                None => (None, None),
            };
            let track = Track {
                key: key.clone(),
                title,
                artist,
                album,
                duration_ms,
                cover_png,
                accent,
                source_app: source,
            };
            self.last_key = Some(key);
            self.last_meta = Some(meta);
            self.last_playback =
                Some((playback.status, playback.position_ms, playback.duration_ms));
            let event = MediaEvent::Track { track, playback };
            self.send(event);
        } else {
            let signature = (playback.status, playback.position_ms, playback.duration_ms);
            if self.last_playback != Some(signature) {
                self.last_playback = Some(signature);
                self.send(MediaEvent::Playback(playback));
            }
        }
    }

    fn previous_duration(&self, title: &str, artist: &str) -> Option<i64> {
        if let Some((_, previous_title, previous_artist, _)) = &self.last_meta {
            if previous_title == title && previous_artist == artist {
                return self
                    .last_playback
                    .map(|(_, _, duration)| duration)
                    .filter(|value| *value > 0);
            }
        }
        None
    }

    fn cover_for(
        &self,
        properties: &windows::Media::Control::GlobalSystemMediaTransportControlsSessionMediaProperties,
        key: &str,
    ) -> Option<(Vec<u8>, Option<String>)> {
        if let Some((bytes, accent)) = cache::read_art(&self.cache_dir, key) {
            return Some((bytes, accent));
        }
        let thumbnail = properties.Thumbnail().ok()?;
        let stream = thumbnail.OpenReadAsync().ok()?.join().ok()?;
        let (bytes, accent) = decode_thumbnail(&stream).ok()?;
        if bytes.len() > 1_500_000 {
            return None;
        }
        cache::write_art(&self.cache_dir, key, &bytes, accent.as_deref());
        Some((bytes, accent))
    }

    fn subscribe(&mut self, session: &GlobalSystemMediaTransportControlsSession, source: &str) {
        if let Some(old) = self.session.take() {
            remove_subscription(&old);
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

    fn none(&mut self, event: MediaEvent) {
        let had_track = self.last_key.is_some();
        self.last_meta = None;
        self.last_key = None;
        self.last_playback = None;
        if had_track {
            self.send(event);
        }
    }
}

fn status_value(status: PlaybackStatusApi) -> PlaybackStatus {
    if status == PlaybackStatusApi::Playing {
        PlaybackStatus::Playing
    } else if status == PlaybackStatusApi::Paused
        || status == PlaybackStatusApi::Opened
        || status == PlaybackStatusApi::Changing
    {
        PlaybackStatus::Paused
    } else {
        PlaybackStatus::Stopped
    }
}

fn timeline_values(
    timeline: &windows::Media::Control::GlobalSystemMediaTransportControlsSessionTimelineProperties,
    status: &PlaybackStatusApi,
) -> (i64, i64) {
    let start = timeline
        .StartTime()
        .map(|value| value.Duration)
        .unwrap_or(0);
    let end = timeline.EndTime().map(|value| value.Duration).unwrap_or(0);
    let raw_position = timeline.Position().map(|value| value.Duration).unwrap_or(0);
    let duration = (end - start).max(0) / 10_000;
    let mut position = (raw_position - start).max(0) / 10_000;

    if *status == PlaybackStatusApi::Playing {
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

    let output = InMemoryRandomAccessStream::new().map_err(|error| error.to_string())?;
    let encoder = BitmapEncoder::CreateAsync(
        BitmapEncoder::PngEncoderId().map_err(|error| error.to_string())?,
        &output,
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

    let size = output.Size().map_err(|error| error.to_string())?;
    if size == 0 || size > 1_500_000 {
        return Err("bad thumbnail size".to_string());
    }
    let input = output
        .GetInputStreamAt(0)
        .map_err(|error| error.to_string())?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playback_status_mapping_is_conservative() {
        assert_eq!(
            status_value(PlaybackStatusApi::Playing),
            PlaybackStatus::Playing
        );
        assert_eq!(
            status_value(PlaybackStatusApi::Paused),
            PlaybackStatus::Paused
        );
        assert_eq!(
            status_value(PlaybackStatusApi::Opened),
            PlaybackStatus::Paused
        );
        assert_eq!(
            status_value(PlaybackStatusApi::Closed),
            PlaybackStatus::Stopped
        );
    }

    #[test]
    fn app_filters_are_case_insensitive() {
        let mut config = Config::default();
        config.behavior.ignored_apps = vec!["chrome.exe".to_string()];
        assert!(!app_allowed("Chrome.EXE", &config));
        config.behavior.ignored_apps.clear();
        config.behavior.preferred_apps = vec!["spotify.exe".to_string()];
        assert!(app_allowed("Spotify.EXE", &config));
        assert!(!app_allowed("musicbee.exe", &config));
    }
}
