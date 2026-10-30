use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::state::AppState;
use tauri::{AppHandle, Manager};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TimerKind {
    IdleDestroy,
    PausedHide,
    PeekHide,
    HiddenDestroy,
}

struct Inner {
    deadlines: Mutex<HashMap<TimerKind, (u64, Instant)>>,
    signal: Condvar,
    shutdown: AtomicBool,
}

#[derive(Clone)]
pub struct TimerHandle {
    inner: Arc<Inner>,
}

impl TimerHandle {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Inner {
                deadlines: Mutex::new(HashMap::new()),
                signal: Condvar::new(),
                shutdown: AtomicBool::new(false),
            }),
        }
    }

    pub fn start(&self, app: AppHandle) {
        let inner = self.inner.clone();
        thread::Builder::new()
            .name("iceberg-timers".into())
            .spawn(move || timer_loop(inner, app))
            .ok();
    }

    pub fn set(&self, kind: TimerKind, after: Duration) {
        let Ok(mut map) = self.inner.deadlines.lock() else {
            return;
        };
        let version = map.get(&kind).map(|(v, _)| *v + 1).unwrap_or(1);
        map.insert(kind, (version, Instant::now() + after));
        self.inner.signal.notify_all();
    }

    pub fn cancel(&self, kind: TimerKind) {
        let Ok(mut map) = self.inner.deadlines.lock() else {
            return;
        };
        map.remove(&kind);
        self.inner.signal.notify_all();
    }

    pub fn shutdown(&self) {
        self.inner.shutdown.store(true, Ordering::Relaxed);
        self.inner.signal.notify_all();
    }
}

fn timer_loop(inner: Arc<Inner>, app: AppHandle) {
    loop {
        let Ok(mut map) = inner.deadlines.lock() else {
            return;
        };
        if inner.shutdown.load(Ordering::Relaxed) {
            return;
        }
        let now = Instant::now();
        let next = map.values().map(|(_, deadline)| *deadline).min();
        let wait = next
            .map(|deadline| deadline.saturating_duration_since(now))
            .unwrap_or(Duration::from_secs(3600));
        let (guard, _) = match inner.signal.wait_timeout(map, wait) {
            Ok(pair) => pair,
            Err(_) => return,
        };
        map = guard;
        if inner.shutdown.load(Ordering::Relaxed) {
            return;
        }
        let now = Instant::now();
        let due: Vec<TimerKind> = map
            .iter()
            .filter(|(_, (_, deadline))| *deadline <= now)
            .map(|(kind, _)| *kind)
            .collect();
        for kind in &due {
            map.remove(kind);
        }
        drop(map);
        for kind in due {
            fire(&app, kind);
        }
    }
}

fn fire(app: &AppHandle, kind: TimerKind) {
    let state = app.state::<AppState>();
    match kind {
        TimerKind::IdleDestroy => {
            let status = state
                .playback
                .lock()
                .map(|p| p.status.clone())
                .unwrap_or_default();
            if status != "playing"
                && !state.settings_open.load(Ordering::SeqCst)
                && !state.peeking.load(Ordering::SeqCst)
            {
                crate::window::destroy_widget(app);
            }
        }
        TimerKind::PausedHide => {
            let status = state
                .playback
                .lock()
                .map(|p| p.status.clone())
                .unwrap_or_default();
            if status == "paused" {
                state.paused_hidden.store(true, Ordering::SeqCst);
                crate::window::update_visibility(app);
            }
        }
        TimerKind::PeekHide => {
            state.peeking.store(false, Ordering::SeqCst);
            crate::window::update_visibility(app);
        }
        TimerKind::HiddenDestroy => {
            if let Some(window) = app.get_webview_window(crate::window::WIDGET_LABEL) {
                if !window.is_visible().unwrap_or(false) {
                    crate::window::destroy_widget(app);
                }
            }
        }
    }
}
