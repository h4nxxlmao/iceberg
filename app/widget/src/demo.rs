use iceberg_core::model::{LyricLine, Lyrics, LyricsKind, Playback, PlaybackStatus, Track};
use iceberg_core::util::{now_ms, track_key};

pub fn track() -> Track {
    Track {
        key: track_key("Midnight City", "M83", "Hurry Up, We're Dreaming", 244_000),
        title: "Midnight City".to_string(),
        artist: "M83".to_string(),
        album: "Hurry Up, We're Dreaming".to_string(),
        duration_ms: 244_000,
        cover_png: None,
        accent: Some("#f07a4f".to_string()),
        source_app: "Iceberg demo".to_string(),
    }
}

pub fn playback() -> Playback {
    Playback {
        status: PlaybackStatus::Playing,
        position_ms: 74_000,
        duration_ms: 244_000,
        received_at_ms: now_ms(),
        source_app: "Iceberg demo".to_string(),
    }
}

pub fn lyrics() -> Lyrics {
    let lines = vec![
        (60_000, "Waiting in a car"),
        (66_000, "Waiting for a ride in the dark"),
        (72_000, "The night city grows"),
        (78_000, "Look and see her eyes, they glow"),
        (84_000, "Waiting in a car"),
        (90_000, "Waiting for a ride in the dark"),
        (96_000, "Drinking in the lounge"),
        (102_000, "Following the neon signs"),
    ]
    .into_iter()
    .map(|(time, text)| LyricLine {
        t_ms: time,
        text: text.to_string(),
        words: Vec::new(),
    })
    .collect();
    Lyrics {
        track_key: track().key,
        kind: LyricsKind::Synced,
        lines,
        plain: String::new(),
        source: "demo".to_string(),
        message: String::new(),
    }
}

pub fn plain_lyrics() -> Lyrics {
    Lyrics {
        track_key: track().key,
        kind: LyricsKind::Plain,
        lines: Vec::new(),
        plain: "The city hums beneath the wires\nThe night is long, the room is quiet\nWe keep the windows open\nAnd let the traffic read the lines\nThe city hums beneath the wires"
            .to_string(),
        source: "demo".to_string(),
        message: String::new(),
    }
}

pub fn arabic_track() -> Track {
    Track {
        key: track_key("ليلة المدينة", "فنان", "", 180_000),
        title: "ليلة المدينة".to_string(),
        artist: "فنان".to_string(),
        album: String::new(),
        duration_ms: 180_000,
        cover_png: None,
        accent: Some("#c8dde2".to_string()),
        source_app: "Iceberg demo".to_string(),
    }
}

pub fn arabic_playback() -> Playback {
    Playback {
        status: PlaybackStatus::Playing,
        position_ms: 4_000,
        duration_ms: 180_000,
        received_at_ms: now_ms(),
        source_app: "Iceberg demo".to_string(),
    }
}

pub fn arabic_lyrics() -> Lyrics {
    let lines = vec![
        (0, "ننتظر في السيارة"),
        (3_000, "ننتظر جولة في الظلام"),
        (6_000, "المدينة تنمو ليلاً"),
        (9_000, "انظر إلى عينيها وهي تلمع"),
        (12_000, "ننتظر في السيارة"),
    ]
    .into_iter()
    .map(|(time, text)| LyricLine {
        t_ms: time,
        text: text.to_string(),
        words: Vec::new(),
    })
    .collect();
    Lyrics {
        track_key: arabic_track().key,
        kind: LyricsKind::Synced,
        lines,
        plain: String::new(),
        source: "demo".to_string(),
        message: String::new(),
    }
}
