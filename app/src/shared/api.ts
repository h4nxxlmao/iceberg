import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  Config,
  Diagnostics,
  EventConfigChanged,
  EventLyricsLoaded,
  EventOffset,
  EventTrackChanged,
  SearchResult,
  Snapshot,
} from "./types";

export const api = {
  snapshot: () => invoke<Snapshot>("get_snapshot"),
  setConfig: (config: Config) => invoke<void>("set_config", { config }),
  resetConfig: () => invoke<Config>("reset_config"),
  applyTheme: (theme: string) => invoke<Config>("apply_theme", { theme }),
  setMode: (mode: string) => invoke<void>("set_mode", { mode }),
  widgetReady: () => invoke<void>("widget_ready"),
  setHover: (hovered: boolean) => invoke<void>("set_widget_hover", { hovered }),
  setExpandedForTrack: () => invoke<void>("expand_for_track"),
  adjustOffset: (delta: number) => invoke<number>("adjust_offset", { delta }),
  resetSongOffset: () => invoke<number>("reset_song_offset"),
  searchLyrics: (query: string) => invoke<SearchResult[]>("search_lyrics", { query }),
  applyLyricsChoice: (id: number) => invoke<void>("apply_lyrics_choice", { id }),
  reapplyLyrics: () => invoke<void>("reapply_lyrics"),
  importLrc: () => invoke<string>("import_lrc"),
  diagnostics: () => invoke<Diagnostics>("get_diagnostics"),
  clearCache: () => invoke<number>("clear_cache"),
  openConfigFolder: () => invoke<void>("open_config_folder"),
  openSettings: () => invoke<void>("open_settings"),
  closeSettings: () => invoke<void>("close_settings"),
  exportTheme: () => invoke<string>("export_theme"),
  importTheme: () => invoke<string>("import_theme"),
  copyText: (text: string) => invoke<void>("copy_text", { text }),
};

export const events = {
  trackChanged: (cb: (p: EventTrackChanged) => void): Promise<UnlistenFn> =>
    listen<EventTrackChanged>("track-changed", (e) => cb(e.payload)),
  positionSync: (cb: (p: import("./types").Playback) => void): Promise<UnlistenFn> =>
    listen<import("./types").Playback>("position-sync", (e) => cb(e.payload)),
  lyricsLoaded: (cb: (p: EventLyricsLoaded) => void): Promise<UnlistenFn> =>
    listen<EventLyricsLoaded>("lyrics-loaded", (e) => cb(e.payload)),
  configChanged: (cb: (p: EventConfigChanged) => void): Promise<UnlistenFn> =>
    listen<EventConfigChanged>("config-changed", (e) => cb(e.payload)),
  offset: (cb: (p: EventOffset) => void): Promise<UnlistenFn> =>
    listen<EventOffset>("offset-indicator", (e) => cb(e.payload)),
  hideOverlay: (cb: () => void): Promise<UnlistenFn> =>
    listen<void>("widget-hidden", () => cb()),
};
