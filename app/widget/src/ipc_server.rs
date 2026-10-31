use std::fs::File;
use std::io::BufReader;
use std::os::windows::io::FromRawHandle;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use iceberg_core::config::Config;
use iceberg_core::ipc::{self, IpcMessage, IpcStatus};
use iceberg_core::presets::builtin_presets;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, GetLastError, ERROR_PIPE_CONNECTED, HANDLE};
use windows::Win32::Storage::FileSystem::PIPE_ACCESS_DUPLEX;
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, NAMED_PIPE_MODE, PIPE_READMODE_BYTE, PIPE_TYPE_BYTE,
    PIPE_WAIT,
};
use winit::event_loop::EventLoopProxy;

use crate::app::AppEvent;

pub fn start(
    proxy: EventLoopProxy<AppEvent>,
    config: Arc<Mutex<Config>>,
    status: Arc<Mutex<IpcStatus>>,
    config_path: PathBuf,
) {
    std::thread::Builder::new()
        .name("iceberg-ipc".into())
        .spawn(move || serve(proxy, config, status, config_path))
        .ok();
}

fn serve(
    proxy: EventLoopProxy<AppEvent>,
    config: Arc<Mutex<Config>>,
    status: Arc<Mutex<IpcStatus>>,
    config_path: PathBuf,
) {
    let name = ipc::pipe_name();
    let mut wide: Vec<u16> = name.encode_utf16().collect();
    wide.push(0);
    loop {
        // SAFETY: the name is a valid NUL terminated UTF-16 string and the returned
        // handle is owned by this thread.
        let handle: HANDLE = unsafe {
            CreateNamedPipeW(
                PCWSTR(wide.as_ptr()),
                PIPE_ACCESS_DUPLEX,
                NAMED_PIPE_MODE(PIPE_TYPE_BYTE.0 | PIPE_READMODE_BYTE.0 | PIPE_WAIT.0),
                1,
                0,
                0,
                0,
                None,
            )
        };
        if handle.is_invalid() {
            std::thread::sleep(Duration::from_millis(100));
            continue;
        }
        // SAFETY: the pipe handle is live and this thread owns it.
        let connected = unsafe { ConnectNamedPipe(handle, None) }.is_ok()
            || unsafe { GetLastError() } == ERROR_PIPE_CONNECTED;
        if !connected {
            // SAFETY: the handle is owned and not connected.
            unsafe {
                let _ = CloseHandle(handle);
            }
            continue;
        }
        // SAFETY: the connected pipe handle is transferred into File exactly once.
        let writer = unsafe { File::from_raw_handle(handle.0 as _) };
        let Ok(reader_file) = writer.try_clone() else {
            continue;
        };
        let mut reader = BufReader::new(reader_file);
        let mut writer = writer;
        let mut opened = false;
        while let Ok(Some(message)) = ipc::read_message(&mut reader) {
            let (response, close_after) =
                handle_message(message, &proxy, &config, &status, &config_path, &mut opened);
            if ipc::write_message(&mut writer, &response).is_err() || close_after {
                break;
            }
        }
        if opened {
            let _ = proxy.send_event(AppEvent::SettingsOpen(false));
        }
    }
}

fn handle_message(
    message: IpcMessage,
    proxy: &EventLoopProxy<AppEvent>,
    config_shared: &Arc<Mutex<Config>>,
    status_shared: &Arc<Mutex<IpcStatus>>,
    config_path: &Path,
    opened: &mut bool,
) -> (IpcMessage, bool) {
    match message {
        IpcMessage::Ping | IpcMessage::Hello { .. } => (IpcMessage::Pong, false),
        IpcMessage::GetConfig => match config_shared.lock() {
            Ok(config) => (
                IpcMessage::ConfigChanged {
                    config: config.clone(),
                },
                false,
            ),
            Err(_) => ipc_error("The widget config is unavailable"),
        },
        IpcMessage::GetStatus => match status_shared.lock() {
            Ok(status) => (
                IpcMessage::Status {
                    status: status.clone(),
                },
                false,
            ),
            Err(_) => ipc_error("The widget status is unavailable"),
        },
        IpcMessage::GetPresets => (
            IpcMessage::Presets {
                presets: builtin_presets(),
            },
            false,
        ),
        IpcMessage::SetConfig { mut config } => {
            config.validate();
            if let Err(error) = config.save(config_path) {
                return ipc_error(&format!("Could not save the config: {error}"));
            }
            if let Ok(mut shared) = config_shared.lock() {
                *shared = config.clone();
            }
            let _ = proxy.send_event(AppEvent::Config(config.clone()));
            (IpcMessage::ConfigChanged { config }, false)
        }
        IpcMessage::SetMode { mode } => {
            let mut config = match config_shared.lock() {
                Ok(config) => config.clone(),
                Err(_) => return ipc_error("The widget config is unavailable"),
            };
            config.mode = mode;
            if let Err(error) = config.save(config_path) {
                return ipc_error(&format!("Could not save the config: {error}"));
            }
            if let Ok(mut shared) = config_shared.lock() {
                *shared = config.clone();
            }
            let _ = proxy.send_event(AppEvent::Config(config.clone()));
            (IpcMessage::ConfigChanged { config }, false)
        }
        IpcMessage::AdjustOffset { delta_ms } => {
            adjust_offset(delta_ms, proxy, config_shared, status_shared, config_path)
        }
        IpcMessage::ResetOffset => {
            adjust_offset(i64::MIN, proxy, config_shared, status_shared, config_path)
        }
        IpcMessage::OpenSettings => {
            *opened = true;
            let _ = proxy.send_event(AppEvent::SettingsOpen(true));
            (IpcMessage::Pong, false)
        }
        IpcMessage::CloseSettings => {
            *opened = false;
            let _ = proxy.send_event(AppEvent::SettingsOpen(false));
            (IpcMessage::Pong, true)
        }
        IpcMessage::ShowWidget => {
            let _ = proxy.send_event(AppEvent::ShowWidget);
            (IpcMessage::Pong, false)
        }
        IpcMessage::FontsChanged => {
            let _ = proxy.send_event(AppEvent::FontsChanged);
            (IpcMessage::Pong, false)
        }
        IpcMessage::RefreshLyrics => {
            let _ = proxy.send_event(AppEvent::RefreshLyrics);
            (IpcMessage::Pong, false)
        }
        IpcMessage::Visibility { visible } => {
            let _ = proxy.send_event(AppEvent::Visibility(visible));
            (IpcMessage::Pong, false)
        }
        other => ipc_error(&format!("Unsupported IPC message: {other:?}")),
    }
}

fn adjust_offset(
    delta_ms: i64,
    proxy: &EventLoopProxy<AppEvent>,
    config_shared: &Arc<Mutex<Config>>,
    status_shared: &Arc<Mutex<IpcStatus>>,
    config_path: &Path,
) -> (IpcMessage, bool) {
    let track_key = match status_shared.lock() {
        Ok(status) => status.track.as_ref().map(|track| track.key.clone()),
        Err(_) => None,
    };
    let Some(track_key) = track_key else {
        return ipc_error("No current track");
    };
    let mut config = match config_shared.lock() {
        Ok(config) => config.clone(),
        Err(_) => return ipc_error("The widget config is unavailable"),
    };
    if delta_ms == i64::MIN {
        config.set_song_offset(&track_key, 0);
    } else {
        let current = config.per_song_offset(&track_key);
        config.set_song_offset(&track_key, current + delta_ms);
    }
    if let Err(error) = config.save(config_path) {
        return ipc_error(&format!("Could not save the offset: {error}"));
    }
    if let Ok(mut shared) = config_shared.lock() {
        *shared = config.clone();
    }
    let _ = proxy.send_event(AppEvent::Config(config.clone()));
    (IpcMessage::ConfigChanged { config }, false)
}

fn ipc_error(message: &str) -> (IpcMessage, bool) {
    (
        IpcMessage::Error {
            message: message.to_string(),
        },
        false,
    )
}
