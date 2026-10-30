use serde::{Deserialize, Serialize};
use std::io::{BufRead, Write};

use crate::config::{Config, WidgetMode};
use crate::model::{Lyrics, Playback, Track};

/// Line-delimited JSON protocol between the widget (server) and settings (client).
/// The transport is a per-session named pipe; this module owns the schema and framing.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(clippy::large_enum_variant)] // Messages are transient JSON frames, not collection values.
pub enum IpcMessage {
    Hello {
        role: String,
        version: String,
    },
    Ping,
    Pong,
    Snapshot {
        config: Config,
        track: Option<Track>,
        playback: Playback,
        lyrics: Option<Lyrics>,
    },
    ConfigChanged {
        config: Config,
    },
    SetConfig {
        config: Config,
    },
    SetMode {
        mode: WidgetMode,
    },
    AdjustOffset {
        delta_ms: i64,
    },
    ResetOffset,
    OpenSettings,
    Visibility {
        visible: bool,
    },
    Quit,
}

pub fn pipe_name() -> String {
    format!(r"\\.\pipe\iceberg-{}", session_id())
}

fn session_id() -> u32 {
    let mut id = 0u32;
    // SAFETY: the output pointer is valid and the call does not retain it.
    unsafe {
        let _ = windows::Win32::System::RemoteDesktop::ProcessIdToSessionId(
            windows::Win32::System::Threading::GetCurrentProcessId(),
            &mut id,
        );
    }
    id
}

pub fn encode(message: &IpcMessage) -> Result<String, serde_json::Error> {
    serde_json::to_string(message)
}

pub fn decode(line: &str) -> Result<IpcMessage, serde_json::Error> {
    serde_json::from_str(line)
}

pub fn write_message<W: Write>(writer: &mut W, message: &IpcMessage) -> std::io::Result<()> {
    let line = encode(message)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    writer.write_all(line.as_bytes())?;
    writer.write_all(b"\n")?;
    writer.flush()
}

pub fn read_message<R: BufRead>(reader: &mut R) -> std::io::Result<Option<IpcMessage>> {
    let mut line = String::new();
    let read = reader.read_line(&mut line)?;
    if read == 0 {
        return Ok(None);
    }
    let message = decode(line.trim_end())
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    Ok(Some(message))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_roundtrip_over_a_line() {
        let message = IpcMessage::AdjustOffset { delta_ms: 100 };
        let mut buffer = Vec::new();
        write_message(&mut buffer, &message).unwrap();
        let mut reader = std::io::Cursor::new(buffer);
        assert_eq!(read_message(&mut reader).unwrap(), Some(message));
        assert_eq!(read_message(&mut reader).unwrap(), None);
    }

    #[test]
    fn config_message_roundtrips() {
        let message = IpcMessage::SetConfig {
            config: Config::default(),
        };
        let encoded = encode(&message).unwrap();
        assert_eq!(decode(&encoded).unwrap(), message);
    }

    #[test]
    fn pipe_name_is_per_session() {
        let name = pipe_name();
        assert!(name.starts_with(r"\\.\pipe\iceberg-"));
    }
}
