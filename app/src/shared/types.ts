export type Mode = "island" | "card" | "lyrics";
export type Effect = "acrylic" | "mica" | "none";
export type LyricAnimation = "fade" | "slide" | "karaoke";
export type PlaybackStatus = "playing" | "paused" | "stopped" | "none";
export type LyricsKind = "synced" | "plain" | "instrumental" | "none" | "error";

export interface Appearance {
  font_family: string;
  font_size: number;
  font_weight: number;
  text_color: string;
  accent_color: string;
  background_color: string;
  opacity: number;
  radius: number;
  effect: Effect;
  accent_from_art: boolean;
  lyric_animation: LyricAnimation;
  lyric_lines: number;
  align: "left" | "center";
  show_cover: boolean;
  show_progress: boolean;
  show_title: boolean;
  show_artist: boolean;
  border: boolean;
  shadow: boolean;
  reduce_motion: boolean;
}

export interface LyricsSettings {
  offset_ms: number;
  unsynced: "static" | "hide";
  cjk_fallback: boolean;
}

export interface Behavior {
  start_with_windows: boolean;
  idle_destroy_minutes: number;
  auto_hide_paused_seconds: number;
  hide_on_fullscreen: boolean;
  click_through: boolean;
  edge_snap: boolean;
  preferred_apps: string[];
  ignored_apps: string[];
  battery_saver: boolean;
  fade_after_seconds: number;
  quick_peek_seconds: number;
}

export interface Hotkeys {
  toggle: string;
  click_through: string;
  offset_up: string;
  offset_down: string;
  mode: string;
  peek: string;
  settings: string;
}

export interface Config {
  version: number;
  theme: string;
  mode: Mode;
  appearance: Appearance;
  lyrics: LyricsSettings;
  behavior: Behavior;
  hotkeys: Hotkeys;
  per_song_offsets: Record<string, number>;
}

export interface Track {
  key: string;
  title: string;
  artist: string;
  album: string;
  duration_ms: number;
  cover: string | null;
  accent: string | null;
  source_app: string;
}

export interface Playback {
  status: PlaybackStatus;
  position_ms: number;
  duration_ms: number;
  received_at_ms: number;
  source_app: string;
}

export interface LyricLine {
  t: number;
  text: string;
}

export interface Lyrics {
  track_key: string;
  kind: LyricsKind;
  lines: LyricLine[];
  plain: string;
  source: string;
  message: string;
}

export interface Snapshot {
  config: Config;
  track: Track | null;
  playback: Playback;
  lyrics: Lyrics | null;
  effective_offset_ms: number;
  cache_bytes: number;
  app_version: string;
}

export interface Diagnostics {
  ram_bytes: number;
  private_bytes: number;
  process_count: number;
  webview_cache_bytes: number;
  lyrics_cache_bytes: number;
  config_path: string;
  webview_path: string;
  active_source: string;
  active_title: string;
  cpu_note: string;
}

export interface SearchResult {
  id: number;
  track_name: string;
  artist_name: string;
  album_name: string;
  duration: number;
  instrumental: boolean;
  has_synced: boolean;
  has_plain: boolean;
}

export interface EventTrackChanged {
  track: Track | null;
  playback: Playback;
}

export interface EventLyricsLoaded {
  lyrics: Lyrics;
  effective_offset_ms: number;
}

export interface EventConfigChanged {
  config: Config;
  effective_offset_ms: number;
}

export interface EventOffset {
  offset_ms: number;
  per_song: boolean;
}

export const DEFAULT_CONFIG: Config = {
  version: 1,
  theme: "dark-glass",
  mode: "card",
  appearance: {
    font_family: "Segoe UI Variable Display",
    font_size: 14,
    font_weight: 400,
    text_color: "#ECEAE1",
    accent_color: "#F07A4F",
    background_color: "#0C1012",
    opacity: 0.92,
    radius: 6,
    effect: "none",
    accent_from_art: false,
    lyric_animation: "slide",
    lyric_lines: 4,
    align: "left",
    show_cover: true,
    show_progress: true,
    show_title: true,
    show_artist: true,
    border: true,
    shadow: false,
    reduce_motion: false,
  },
  lyrics: {
    offset_ms: 0,
    unsynced: "static",
    cjk_fallback: true,
  },
  behavior: {
    start_with_windows: false,
    idle_destroy_minutes: 5,
    auto_hide_paused_seconds: 30,
    hide_on_fullscreen: true,
    click_through: false,
    edge_snap: true,
    preferred_apps: [],
    ignored_apps: [],
    battery_saver: false,
    fade_after_seconds: 0,
    quick_peek_seconds: 5,
  },
  hotkeys: {
    toggle: "Ctrl+Alt+KeyI",
    click_through: "Ctrl+Alt+KeyL",
    offset_up: "Ctrl+Alt+ArrowRight",
    offset_down: "Ctrl+Alt+ArrowLeft",
    mode: "Ctrl+Alt+KeyM",
    peek: "Ctrl+Alt+KeyP",
    settings: "Ctrl+Alt+Comma",
  },
  per_song_offsets: {},
};

export const FONT_CHOICES = [
  { label: "Segoe UI Variable", value: "Segoe UI Variable Display" },
  { label: "Segoe UI", value: "Segoe UI" },
  { label: "Bahnschrift", value: "Bahnschrift" },
  { label: "Cascadia Mono", value: "Cascadia Mono" },
  { label: "Consolas", value: "Consolas" },
  { label: "Georgia", value: "Georgia" },
];

export const THEME_NAMES = [
  { id: "dark-glass", label: "Glacier" },
  { id: "light", label: "Paper" },
  { id: "minimal", label: "Graphite" },
  { id: "neon", label: "Ember" },
  { id: "mono", label: "Frost" },
  { id: "album-color", label: "Cover" },
];
