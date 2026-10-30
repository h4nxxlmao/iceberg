mod apply;
mod cache;
mod commands;
mod config;
mod fullscreen;
mod hotkeys;
mod lyrics;
mod perf;
mod smtc;
mod state;
mod timers;
mod tray;
mod util;
mod window;

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::Mutex;

use tauri::{Manager, RunEvent};

use cache::CACHE_CAP;
use config::{Config, ConfigStore};
use lyrics::{LyricLine, Lyrics, LyricsKind};
use state::{AppState, Playback, Track, WindowMemory};
use timers::TimerHandle;
use util::now_ms;

struct Paths {
    config: PathBuf,
    data: PathBuf,
}

fn resolve_paths() -> Paths {
    let args: Vec<String> = std::env::args().collect();
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(PathBuf::from));
    let portable = args.iter().any(|arg| arg == "--portable")
        || exe_dir
            .as_ref()
            .map(|dir| dir.join("portable.txt").is_file())
            .unwrap_or(false);

    let roaming = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("IcebergRoaming"));
    let local = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("IcebergLocal"));

    if portable {
        let base = exe_dir.unwrap_or_else(|| roaming.join("Iceberg"));
        Paths {
            config: base.join("config.json"),
            data: base.join("data"),
        }
    } else {
        Paths {
            config: roaming.join("Iceberg").join("config.json"),
            data: local.join("Iceberg"),
        }
    }
}

fn load_window_memory(path: &PathBuf) -> WindowMemory {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn seed_demo(state: &AppState) {
    let track = Track {
        key: "demo|midnight city|m83|244".to_string(),
        title: "Midnight City".to_string(),
        artist: "M83".to_string(),
        album: "Hurry Up, We're Dreaming".to_string(),
        duration_ms: 244_000,
        cover: None,
        accent: Some("#f472b6".to_string()),
        source_app: "Iceberg demo".to_string(),
    };
    let playback = Playback {
        status: "playing".to_string(),
        position_ms: 74_000,
        duration_ms: 244_000,
        received_at_ms: now_ms(),
        source_app: "Iceberg demo".to_string(),
    };
    let lines = vec![
        LyricLine {
            t: 60_000,
            text: "Waiting in a car".to_string(),
        },
        LyricLine {
            t: 66_000,
            text: "Waiting for a ride in the dark".to_string(),
        },
        LyricLine {
            t: 72_000,
            text: "The night city grows".to_string(),
        },
        LyricLine {
            t: 78_000,
            text: "Look and see her eyes, they glow".to_string(),
        },
        LyricLine {
            t: 84_000,
            text: "Waiting in a car".to_string(),
        },
        LyricLine {
            t: 90_000,
            text: "Waiting for a ride in the dark".to_string(),
        },
    ];
    let lyrics = Lyrics {
        track_key: track.key.clone(),
        kind: LyricsKind::Synced,
        lines,
        plain: String::new(),
        source: "demo".to_string(),
        message: String::new(),
    };
    state.set_playback(playback);
    state.set_track(Some(track));
    state.set_lyrics(Some(lyrics));
}

pub fn run() {
    let paths = resolve_paths();
    if let Some(parent) = paths.config.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::create_dir_all(&paths.data);
    let cache_dir = paths.data.join("cache");
    let webview_dir = paths.data.join("webview");
    let window_path = paths.data.join("window.json");
    let _ = fs::create_dir_all(&cache_dir);
    let _ = fs::create_dir_all(&webview_dir);
    let _ = fs::create_dir_all(cache::lyrics_dir(&cache_dir));
    let _ = fs::create_dir_all(cache::art_dir(&cache_dir));
    let _ = fs::create_dir_all(cache::override_dir(&cache_dir));

    cache::trim_webview_cache(&webview_dir, CACHE_CAP);
    cache::evict(&cache_dir, CACHE_CAP);

    let mut config = Config::load(&paths.config);
    config.validate();
    if !paths.config.exists() {
        let _ = config.save(&paths.config);
    }

    let window_memory = load_window_memory(&window_path);
    let args: Vec<String> = std::env::args().collect();
    let demo = args.iter().any(|arg| arg == "--demo");
    let show_widget = args.iter().any(|arg| arg == "--show-widget");
    let open_settings = args.iter().any(|arg| arg == "--settings");
    if demo || open_settings {
        config.behavior.hide_on_fullscreen = false;
    }

    let state = AppState {
        config: Mutex::new(config.clone()),
        store: ConfigStore::new(paths.config.clone()),
        config_path: paths.config.clone(),
        cache_dir: cache_dir.clone(),
        webview_dir: webview_dir.clone(),
        window_path,
        window_memory: Mutex::new(window_memory),
        track: Mutex::new(None),
        playback: Mutex::new(Playback::default()),
        lyrics: Mutex::new(None),
        search_results: Mutex::new(Vec::new()),
        effective_offset: AtomicI64::new(0),
        user_hidden: AtomicBool::new(false),
        peeking: AtomicBool::new(show_widget),
        paused_hidden: AtomicBool::new(false),
        fullscreen: AtomicBool::new(false),
        widget_suspended: AtomicBool::new(false),
        widget_loaded: AtomicBool::new(false),
        island_expanded: AtomicBool::new(false),
        settings_open: AtomicBool::new(false),
        demo: AtomicBool::new(demo),
        quitting: AtomicBool::new(false),
        lyrics_generation: AtomicU64::new(0),
        cache_bytes: AtomicU64::new(cache::cache_size(&cache_dir)),
        move_generation: AtomicU64::new(0),
        smtc_poke: Mutex::new(None),
        timers: TimerHandle::new(),
    };

    if demo {
        seed_demo(&state);
    }

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            let state = app.state::<AppState>();
            if state.current_track().is_some() {
                window::peek(app);
            } else {
                window::open_settings(app);
            }
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            commands::get_snapshot,
            commands::set_config,
            commands::reset_config,
            commands::apply_theme,
            commands::set_mode,
            commands::widget_ready,
            commands::set_widget_hover,
            commands::expand_for_track,
            commands::adjust_offset,
            commands::reset_song_offset,
            commands::search_lyrics,
            commands::apply_lyrics_choice,
            commands::reapply_lyrics,
            commands::import_lrc,
            commands::get_diagnostics,
            commands::clear_cache,
            commands::open_config_folder,
            commands::open_settings,
            commands::close_settings,
            commands::export_theme,
            commands::import_theme,
            commands::copy_text,
        ])
        .setup(move |app| {
            app.manage(state);
            perf::set_efficiency_mode();

            let state = app.state::<AppState>();
            let startup = state.config();
            let demo_mode = state.demo.load(Ordering::SeqCst);
            if let Err(error) = tray::init(app.handle()) {
                eprintln!("iceberg: tray unavailable: {error}");
            }
            app.state::<AppState>().timers.start(app.handle().clone());
            apply::apply_config(app.handle(), &startup);
            fullscreen::start(app.handle());

            if demo_mode {
                tray::set_playing(app.handle(), true);
            }
            window::update_visibility(app.handle());

            if open_settings {
                window::open_settings(app.handle());
            }

            if !demo_mode {
                smtc::start(app.handle().clone());
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("Iceberg failed to start");

    app.run(|app, event| match event {
        RunEvent::ExitRequested { api, code, .. } => {
            let quitting = app.state::<AppState>().quitting.load(Ordering::SeqCst);
            if code.is_none() && !quitting {
                api.prevent_exit();
            }
        }
        RunEvent::Exit => {
            let state = app.state::<AppState>();
            window::flush_position(app);
            state.store.flush(&state.config());
            state.timers.shutdown();
            cache::trim_webview_cache(&state.webview_dir, CACHE_CAP);
        }
        _ => {}
    });
}
