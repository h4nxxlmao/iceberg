import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import "./style.css";

type BackgroundKind = "solid" | "transparent" | "acrylic" | "ice" | "liquid";
type Align = "left" | "center";
type TextCase = "original" | "upper" | "lower";
type Transition = "slide" | "fade" | "off";
type SourceMode = "spotify" | "apple_music" | "custom" | "any";
type Anchor =
  | "top_left"
  | "top_center"
  | "top_right"
  | "center_left"
  | "center"
  | "center_right"
  | "bottom_left"
  | "bottom_center"
  | "bottom_right";

interface Background {
  kind: BackgroundKind;
  backdrop: "wallpaper" | "live";
  solid_color: string;
  solid_opacity: number;
  transparent_opacity: number;
  accent_color: string;
  accent_from_art: boolean;
  acrylic_tint: string;
  acrylic_tint_strength: number;
  acrylic_noise: number;
  ice_frost: number;
  ice_clarity: number;
  ice_tint: string;
  liquid_refraction: number;
  liquid_blur: number;
  liquid_highlight: number;
  liquid_bevel: number;
  liquid_chromatic: boolean;
  radius: number;
  border_width: number;
  border_color: string;
  border_opacity: number;
  shadow: boolean;
  shadow_opacity: number;
  padding: number;
}

interface Typography {
  font_family: string;
  font_size: number;
  weight: number;
  italic: boolean;
  letter_spacing: number;
  line_height: number;
  align: Align;
  case: TextCase;
  active_color: string;
  inactive_opacity: number;
  shadow: boolean;
  outline: boolean;
  visible_lines: number;
  transition: Transition;
  transition_ms: number;
}

interface Layout {
  scale: number;
  width: number;
  anchor: Anchor;
  offset_x: number;
  offset_y: number;
  per_monitor: Record<string, unknown>;
}

interface LyricsSettings {
  offset_ms: number;
  unsynced: "static" | "hide";
  word_highlight: boolean;
}

interface Behavior {
  start_with_windows: boolean;
  hide_on_fullscreen: boolean;
  click_through: boolean;
  edge_snap: boolean;
  idle_timeout_seconds: number;
  pause_hide_seconds: number;
  battery_saver: boolean;
  reduce_motion: boolean;
  show_progress: boolean;
  source: SourceMode;
  source_custom: string;
  preferred_apps: string[];
  ignored_apps: string[];
}

interface Hotkeys {
  toggle: string;
  click_through: string;
  offset_up: string;
  offset_down: string;
  mode: string;
  peek: string;
  settings: string;
}

interface SongOverride {
  offset_ms: number | null;
  font_family: string | null;
}

interface Config {
  version: number;
  mode: "card" | "lyrics" | "island";
  background: Background;
  typography: Typography;
  layout: Layout;
  lyrics: LyricsSettings;
  behavior: Behavior;
  hotkeys: Hotkeys;
  per_song: Record<string, SongOverride>;
  active_preset: string | null;
}

interface Preset {
  id: string;
  name: string;
  background: Background;
  typography: Typography;
}

interface TrackInfo {
  key: string;
  title: string;
  artist: string;
  album: string;
  source_app: string;
}

interface IpcStatus {
  track?: TrackInfo | null;
  line?: string | null;
  line_index?: number | null;
  lyrics_status?: string;
  lyrics_message?: string | null;
}

interface FontInfo {
  family: string;
  category: string;
  stock: boolean;
}

interface Bootstrap {
  config: Config;
  status: IpcStatus;
  presets: Preset[];
  fonts: FontInfo[];
  section: string;
}

interface SyncState {
  config: Config;
  status: IpcStatus;
}

interface FontPreview {
  family: string;
  data: string;
}

interface FontUpload {
  family: string;
  fonts: FontInfo[];
}

interface ImageData {
  data: string;
}

type SectionId =
  | "appearance"
  | "typography"
  | "layout"
  | "lyrics"
  | "playback"
  | "hotkeys"
  | "about";

interface SectionInfo {
  id: SectionId;
  label: string;
  icon: string;
}

const SECTIONS: SectionInfo[] = [
  {
    id: "appearance",
    label: "Appearance",
    icon: "surface",
  },
  {
    id: "typography",
    label: "Typography",
    icon: "type",
  },
  {
    id: "layout",
    label: "Layout",
    icon: "layout",
  },
  {
    id: "lyrics",
    label: "Lyrics",
    icon: "lyrics",
  },
  {
    id: "playback",
    label: "Playback",
    icon: "play",
  },
  {
    id: "hotkeys",
    label: "Hotkeys",
    icon: "keys",
  },
  {
    id: "about",
    label: "About",
    icon: "info",
  },
];

const SURFACES: { kind: BackgroundKind; name: string }[] = [
  { kind: "solid", name: "Solid" },
  { kind: "transparent", name: "Transparent" },
  { kind: "acrylic", name: "Acrylic" },
  { kind: "ice", name: "Ice" },
  { kind: "liquid", name: "Liquid" },
];

const ANCHORS: Anchor[] = [
  "top_left",
  "top_center",
  "top_right",
  "center_left",
  "center",
  "center_right",
  "bottom_left",
  "bottom_center",
  "bottom_right",
];

interface UiState {
  config: Config | null;
  presets: Preset[];
  fonts: FontInfo[];
  status: IpcStatus;
  section: SectionId;
  saveState: "idle" | "saving" | "saved" | "error";
  banner: string;
  bannerKind: "error" | "info";
  fontPreviews: Map<string, string>;
  fontPreviewLine: string;
  fontPreviewFailed: boolean;
  images: Map<string, string>;
  uploading: boolean;
  resetAllArmed: boolean;
  version: string;
}

const FONT_SPECIMEN = "The night city grows";
const state: UiState = {
  config: null,
  presets: [],
  fonts: [],
  status: {},
  section: "appearance",
  saveState: "idle",
  banner: "",
  bannerKind: "error",
  fontPreviews: new Map(),
  fontPreviewLine: "",
  fontPreviewFailed: false,
  images: new Map(),
  uploading: false,
  resetAllArmed: false,
  version: "0.1.0",
};

const appRoot = document.querySelector<HTMLDivElement>("#app");
if (!appRoot) {
  throw new Error("Missing app root");
}
const root = appRoot;

let saveTimer: number | undefined;
let savePromise: Promise<void> | null = null;
let saveQueued = false;
let fontPreviewRequest: Promise<void> | null = null;
let pointerActive = false;

window.addEventListener("pointerdown", () => { pointerActive = true; }, true);
window.addEventListener("pointerup", () => { pointerActive = false; }, true);
window.addEventListener("pointercancel", () => { pointerActive = false; }, true);

function escapeHtml(value: unknown): string {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");
}

function escapeAttr(value: unknown): string {
  return escapeHtml(value);
}

function icon(name: string): string {
  const paths: Record<string, string> = {
    surface:
      '<rect x="3.5" y="3.5" width="17" height="17" rx="1.5"/><path d="M3.5 12h17"/><path d="M12 3.5v17"/>',
    type: '<path d="M4 7V4.5h16V7"/><path d="M12 4.5v15"/><path d="M8 19.5h8"/>',
    layout:
      '<rect x="3.5" y="4.5" width="17" height="15" rx="1.5"/><path d="M3.5 9.5h17"/><path d="M9 9.5v10"/>',
    lyrics:
      '<path d="M4 6.5h16"/><path d="M4 12h12"/><path d="M4 17.5h8"/>',
    play: '<circle cx="12" cy="12" r="8.5"/><path d="m10 8.5 6 3.5-6 3.5z"/>',
    keys:
      '<rect x="3.5" y="6" width="17" height="12" rx="1.5"/><path d="M7 10h.01M11 10h.01M15 10h.01M7 14h10"/>',
    info: '<circle cx="12" cy="12" r="8.5"/><path d="M12 11v5"/><path d="M12 8h.01"/>',
    reveal: '<path d="M4 12h15"/><path d="m14 7 5 5-5 5"/>',
    upload: '<path d="M12 16V4"/><path d="m7 9 5-5 5 5"/><path d="M4 20h16"/>',
    close: '<path d="m6 6 12 12"/><path d="m18 6-12 12"/>',
  };
  return `<svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${paths[name] ?? paths.info}</svg>`;
}

function hex6(value: string): string {
  const clean = value.trim().replace(/^#/, "");
  if (clean.length === 3) {
    return `#${clean[0]}${clean[0]}${clean[1]}${clean[1]}${clean[2]}${clean[2]}`;
  }
  if (/^[0-9a-fA-F]{6}$/.test(clean)) {
    return `#${clean.toLowerCase()}`;
  }
  return "#000000";
}

function pct(value: number): string {
  return `${Math.round(value * 100)}%`;
}

function ms(value: number): string {
  return `${Math.round(value)} ms`;
}

function currentTrack(): TrackInfo | null {
  return state.status.track ?? null;
}

function sourceLabel(source: string | undefined): string {
  if (!source) {
    return "";
  }
  const lower = source.toLowerCase();
  if (lower.includes("iceberg demo")) {
    return "Demo preview";
  }
  if (lower.includes("spotify")) {
    return "Spotify";
  }
  if (lower.includes("applemusic") || lower.includes("apple music") || lower.includes("itunes")) {
    return "Apple Music";
  }
  return "Media app";
}

function sectionById(id: string): SectionInfo | undefined {
  return SECTIONS.find((section) => section.id === id);
}

function stateScreen(title: string, message: string, retry: boolean): string {
  return `
    <div class="state-screen">
      <div class="state-brand">Iceberg</div>
      <h1>${escapeHtml(title)}</h1>
      <p>${escapeHtml(message)}</p>
      ${
        retry
          ? '<button class="button primary" id="retry">Try again</button>'
          : '<div class="state-wait">Connecting to the widget</div>'
      }
    </div>
  `;
}

async function start(): Promise<void> {
  root.innerHTML = stateScreen("Opening settings", "Starting the widget and loading the current configuration.", false);
  try {
    const data = await invoke<Bootstrap>("bootstrap");
    state.config = data.config;
    state.status = data.status;
    state.presets = data.presets;
    state.fonts = data.fonts;
    state.section = SECTIONS.some((section) => section.id === data.section)
      ? (data.section as SectionId)
      : "appearance";
    renderShell();
    renderSection();
  } catch (error) {
    showFatal(String(error));
  }
}

function showFatal(message: string): void {
  root.innerHTML = stateScreen("Could not open Iceberg", message, true);
  document.getElementById("retry")?.addEventListener("click", () => {
    void start();
  });
}

function renderShell(): void {
  root.innerHTML = `
    <div class="layout">
      <aside class="rail">
        <div class="brand">
          <div class="brand-name">Iceberg</div>
          <div class="brand-sub">Settings</div>
        </div>
        <nav class="nav" aria-label="Settings sections">
          ${SECTIONS.map(
            (section) => `
              <button class="nav-item" data-section="${section.id}" type="button">
                ${icon(section.icon)}
                <span>${escapeHtml(section.label)}</span>
              </button>
            `,
          ).join("")}
        </nav>
        <div class="rail-foot">
          <div class="status" id="widget-status"></div>
        </div>
      </aside>
      <main class="sheet">
        <header class="sheet-head">
          <div class="head-copy">
            <h1 id="section-title"></h1>
          </div>
          <div class="head-actions">
            <span class="save-state" id="save-state"></span>
            <button class="button secondary" id="reveal-widget" type="button">
              ${icon("reveal")} Reveal widget
            </button>
          </div>
        </header>
        <div class="banner" id="banner" hidden>
          <span id="banner-text"></span>
          <button class="icon-button" id="banner-close" type="button" aria-label="Dismiss">${icon("close")}</button>
        </div>
        <div class="content" id="content"></div>
      </main>
    </div>
  `;
  for (const button of root.querySelectorAll<HTMLButtonElement>("[data-section]")) {
    button.addEventListener("click", () => {
      state.section = button.dataset.section as SectionId;
      renderSection();
    });
  }
  document.getElementById("reveal-widget")?.addEventListener("click", () => {
    void revealWidget();
  });
  document.getElementById("banner-close")?.addEventListener("click", hideBanner);
}

function updateChrome(): void {
  for (const button of root.querySelectorAll<HTMLButtonElement>("[data-section]")) {
    const active = button.dataset.section === state.section;
    button.classList.toggle("is-active", active);
    button.setAttribute("aria-current", active ? "page" : "false");
  }
  const section = sectionById(state.section);
  const title = document.getElementById("section-title");
  if (title) {
    title.textContent = section?.label ?? "Settings";
  }
  updateStatus();
  updateSaveState();
  updateBanner();
}

function updateStatus(): void {
  const element = document.getElementById("widget-status");
  if (!element) {
    return;
  }
  const track = currentTrack();
  const rawLine = state.status.line?.trim();
  const line =
    rawLine ||
    (state.status.lyrics_status === "loading"
      ? "Looking up lyrics"
      : state.status.lyrics_status === "error"
        ? state.status.lyrics_message?.trim() || "Lyrics unavailable"
        : state.status.lyrics_status === "not_found"
          ? "No lyrics found"
          : state.status.lyrics_status === "plain"
            ? "Plain lyrics"
            : state.status.lyrics_status === "instrumental"
              ? "Instrumental"
              : track
                ? "No lyric line"
                : "Preview on the desktop");
  const demo = track?.source_app === "Iceberg demo";
  const source = track ? sourceLabel(track.source_app) : "";
  element.innerHTML = `
    <span class="status-mark ${track ? "is-live" : ""}" aria-hidden="true"></span>
    <div class="status-copy">
      <div class="status-title">${escapeHtml(track ? track.title : "Widget ready")}${demo ? '<span class="status-demo">demo</span>' : ""}</div>
      ${source && !demo ? `<div class="status-source">${escapeHtml(source)}</div>` : ""}
      <div class="status-line">${escapeHtml(line)}</div>
    </div>
  `;
}

function setSaveState(next: UiState["saveState"]): void {
  state.saveState = next;
  updateSaveState();
}

function updateSaveState(): void {
  const element = document.getElementById("save-state");
  if (!element) {
    return;
  }
  const labels = {
    idle: "",
    saving: "Saving",
    saved: "Saved",
    error: "Save failed",
  };
  element.textContent = labels[state.saveState];
  element.className = `save-state ${state.saveState}`;
}

function showBanner(message: string, kind: "error" | "info" = "error"): void {
  state.banner = message;
  state.bannerKind = kind;
  updateBanner();
}

function hideBanner(): void {
  state.banner = "";
  updateBanner();
}

function updateBanner(): void {
  const banner = document.getElementById("banner");
  const text = document.getElementById("banner-text");
  if (!banner || !text) {
    return;
  }
  banner.hidden = state.banner.length === 0;
  banner.className = `banner ${state.bannerKind}`;
  text.textContent = state.banner;
}

function renderSection(): void {
  const config = state.config;
  if (!config) {
    return;
  }
  updateChrome();
  const content = document.getElementById("content");
  if (!content) {
    return;
  }
  content.innerHTML =
    state.section === "appearance"
      ? renderAppearance(config)
      : state.section === "typography"
        ? renderTypography(config)
        : state.section === "layout"
          ? renderLayout(config)
          : state.section === "lyrics"
            ? renderLyrics(config)
            : state.section === "playback"
              ? renderPlayback(config)
              : state.section === "hotkeys"
                ? renderHotkeys(config)
                : renderAbout();
  bindCommon();
  switch (state.section) {
    case "appearance":
      bindAppearance(config);
      hydrateImages();
      break;
    case "typography":
      bindTypography(config);
      void ensureFontPreviews();
      break;
    case "layout":
      bindLayout(config);
      break;
    case "lyrics":
      bindLyrics(config);
      break;
    case "playback":
      bindPlayback(config);
      break;
    case "hotkeys":
      bindHotkeys(config);
      break;
    case "about":
      bindAbout();
      break;
  }
}

function sectionTools(section: SectionId): string {
  return `
    <div class="section-tools">
      <button class="text-button" type="button" data-reset="${section}">Reset section</button>
    </div>
  `;
}

function group(title: string, note: string, body: string): string {
  return `
    <section class="group">
      <div class="group-head"><h2>${escapeHtml(title)}</h2></div>
      ${note ? `<p class="group-note">${escapeHtml(note)}</p>` : ""}
      ${body}
    </section>
  `;
}

function row(label: string, description: string, control: string, id: string): string {
  return `
    <div class="row">
      <div class="row-copy">
        <label for="${id}">${escapeHtml(label)}</label>
        ${description ? `<p>${escapeHtml(description)}</p>` : ""}
      </div>
      <div class="row-control">${control}</div>
    </div>
  `;
}

function rangeRow(
  id: string,
  label: string,
  description: string,
  min: number,
  max: number,
  step: number,
  value: number,
  format: (value: number) => string,
): string {
  return row(
    label,
    description,
    `<div class="range-control"><input id="${id}" type="range" min="${min}" max="${max}" step="${step}" value="${value}"><output id="${id}-out">${escapeHtml(format(value))}</output></div>`,
    id,
  );
}

function colorRow(id: string, label: string, description: string, value: string): string {
  return row(
    label,
    description,
    `<div class="color-control"><input id="${id}" type="color" value="${hex6(value)}"><code id="${id}-out">${escapeHtml(hex6(value))}</code></div>`,
    id,
  );
}

function checkRow(id: string, label: string, description: string, checked: boolean): string {
  return `
    <label class="check-row" for="${id}">
      <input id="${id}" type="checkbox" ${checked ? "checked" : ""}>
      <span class="check-box" aria-hidden="true"></span>
      <span class="check-copy">
        <span class="check-title">${escapeHtml(label)}</span>
        <span class="check-desc">${escapeHtml(description)}</span>
      </span>
    </label>
  `;
}

function choiceRow(
  id: string,
  label: string,
  description: string,
  options: { value: string; label: string }[],
  selected: string,
): string {
  return row(
    label,
    description,
    `<div class="choice-group" id="${id}" role="radiogroup" aria-label="${escapeAttr(label)}">
      ${options
        .map(
          (option) =>
            `<button type="button" role="radio" class="choice ${option.value === selected ? "is-selected" : ""}" aria-checked="${option.value === selected}" data-value="${escapeAttr(option.value)}">${escapeHtml(option.label)}</button>`,
        )
        .join("")}
    </div>`,
    id,
  );
}

function renderAppearance(config: Config): string {
  const background = config.background;
  const surfaceControls: string[] = [];
  if (background.kind === "solid") {
    surfaceControls.push(
      colorRow("bg-solid", "Color", "The card fill behind the lyrics.", background.solid_color),
      rangeRow("bg-solid-opacity", "Opacity", "Slightly translucent over the desktop.", 0, 1, 0.01, background.solid_opacity, pct),
    );
  } else if (background.kind === "transparent") {
    surfaceControls.push(
      rangeRow("bg-transparent-opacity", "Wash", "A tinted sheet over the desktop. Zero leaves it clear.", 0, 1, 0.01, background.transparent_opacity, pct),
    );
  } else if (background.kind === "acrylic") {
    surfaceControls.push(
      colorRow("bg-acrylic-tint", "Tint", "The color mixed into the blurred wallpaper.", background.acrylic_tint),
      rangeRow("bg-acrylic-strength", "Tint strength", "How far the wallpaper moves toward the tint.", 0, 1, 0.01, background.acrylic_tint_strength, pct),
      rangeRow("bg-acrylic-noise", "Grain", "Fine surface grain. Zero is smooth.", 0, 1, 0.01, background.acrylic_noise, pct),
    );
  } else if (background.kind === "ice") {
    surfaceControls.push(
      colorRow("bg-ice-tint", "Tint", "The cool cast on the frosted surface.", background.ice_tint),
      rangeRow("bg-ice-frost", "Frost", "Diffusion and frost grain.", 0, 1, 0.01, background.ice_frost, pct),
      rangeRow("bg-ice-clarity", "Clarity", "A little of the unblurred wallpaper.", 0, 1, 0.01, background.ice_clarity, pct),
    );
  } else {
    surfaceControls.push(
      rangeRow("bg-liquid-refraction", "Refraction", "Edge displacement in the glass.", 0, 1, 0.01, background.liquid_refraction, pct),
      rangeRow("bg-liquid-blur", "Blur", "Wallpaper blur radius.", 0, 64, 1, background.liquid_blur, (value) => `${Math.round(value)} px`),
      rangeRow("bg-liquid-highlight", "Highlight", "The light sheen across the surface.", 0, 1, 0.01, background.liquid_highlight, pct),
      rangeRow("bg-liquid-bevel", "Bevel", "The width of the refracted edge.", 0, 48, 1, background.liquid_bevel, (value) => `${Math.round(value)} px`),
      checkRow("bg-liquid-chromatic", "Chromatic edge", "A small red/blue split at the refracted rim.", background.liquid_chromatic),
    );
  }

  return `
    ${sectionTools("appearance")}
    ${group(
      "Surface",
      "",
      `<div class="surface-list">${SURFACES.map((surface) => {
        const selected = background.kind === surface.kind;
        return `<button class="surface-option ${selected ? "is-selected" : ""}" type="button" data-surface="${surface.kind}" aria-pressed="${selected}">
          <span class="surface-image"><img data-image="surface:${surface.kind}" alt="" loading="lazy"></span>
          <span class="surface-copy"><span class="surface-name">${surface.name}</span></span>
        </button>`;
      }).join("")}</div>`,
    )}
    ${group(
      "Presets",
      "",
      `<div class="preset-grid">${state.presets.map((preset) => {
        const selected = config.active_preset === preset.id;
        return `<button class="preset ${selected ? "is-selected" : ""}" type="button" data-preset="${preset.id}" aria-pressed="${selected}">
          <span class="preset-image"><img data-image="preset:${preset.id}" alt="" loading="lazy"></span>
          <span class="preset-name">${escapeHtml(preset.name)}</span>
        </button>`;
      }).join("")}</div>`,
    )}
    <details class="advanced">
      <summary>Surface details</summary>
      <div class="details-body">
        ${surfaceControls.join("")}
        ${[
          rangeRow("bg-radius", "Radius", "", 0, 48, 1, background.radius, (value) => `${Math.round(value)} px`),
          rangeRow("bg-padding", "Padding", "", 4, 48, 1, background.padding, (value) => `${Math.round(value)} px`),
          rangeRow("bg-border-width", "Border", "", 0, 4, 0.1, background.border_width, (value) => `${value.toFixed(1)} px`),
          colorRow("bg-border-color", "Border color", "", background.border_color),
          rangeRow("bg-border-opacity", "Border opacity", "", 0, 1, 0.01, background.border_opacity, pct),
          checkRow("bg-shadow", "Drop shadow", "", background.shadow),
          rangeRow("bg-shadow-opacity", "Shadow strength", "", 0, 1, 0.01, background.shadow_opacity, pct),
        ].join("")}
      </div>
    </details>
  `;
}

function fontTiles(config: Config): string {
  return `<div class="font-rail">${state.fonts
    .map((font) => {
      const selected = config.typography.font_family === font.family;
      const preview = state.fontPreviews.get(font.family);
      return `<button class="font-tile ${selected ? "is-selected" : ""}" type="button" data-font="${escapeAttr(font.family)}" aria-pressed="${selected}">
        <span class="font-preview">${
          preview
            ? `<img src="${preview}" alt="">`
            : `<span class="font-preview-placeholder">${state.fontPreviewFailed ? "Preview unavailable" : "Rendering specimen"}</span>`
        }</span>
        <span class="font-name">${escapeHtml(font.family)}</span>
        <span class="font-meta">${escapeHtml(font.category)}</span>
      </button>`;
    })
    .join("")}</div>`;
}

function renderTypography(config: Config): string {
  const typography = config.typography;
  const track = currentTrack();
  const trackFont = track ? config.per_song[track.key]?.font_family : null;
  return `
    ${sectionTools("typography")}
    ${group(
      "Font",
      "",
      `${fontTiles(config)}
       <div class="upload-row">
         <button class="button secondary" id="upload-font" type="button" ${state.uploading ? "disabled" : ""}>
           ${icon("upload")} ${state.uploading ? "Adding font..." : "Add font"}
         </button>
         <span class="fine-print">.ttf or .otf</span>
       </div>`,
    )}
    ${group(
      "Type",
      "",
      [
        rangeRow("type-size", "Size", "", 12, 40, 0.5, typography.font_size, (value) => `${value.toFixed(1)} px`),
        rangeRow("type-weight", "Weight", "", 300, 800, 50, typography.weight, (value) => String(Math.round(value))),
        rangeRow("type-line-height", "Line height", "", 1, 2, 0.05, typography.line_height, (value) => value.toFixed(2)),
        choiceRow("type-align", "Alignment", "", [
          { value: "left", label: "Left" },
          { value: "center", label: "Center" },
        ], typography.align),
        choiceRow("type-case", "Case", "", [
          { value: "original", label: "Original" },
          { value: "upper", label: "Upper" },
          { value: "lower", label: "Lower" },
        ], typography.case),
      ].join(""),
    )}
    ${group(
      "Color and lines",
      "",
      [
        colorRow("type-active", "Active color", "", typography.active_color),
        rangeRow("type-visible", "Visible lines", "", 3, 7, 1, typography.visible_lines, (value) => String(Math.round(value))),
      ].join(""),
    )}
    <details class="advanced">
      <summary>Fine tuning</summary>
      <div class="details-body">
        ${[
          rangeRow("type-letter-spacing", "Letter spacing", "", -0.05, 0.2, 0.01, typography.letter_spacing, (value) => `${value.toFixed(2)} em`),
          checkRow("type-italic", "Italic", "", typography.italic),
          rangeRow("type-inactive", "Inactive opacity", "", 0.15, 0.85, 0.01, typography.inactive_opacity, pct),
          checkRow("type-shadow", "Text shadow", "", typography.shadow),
          checkRow("type-outline", "Text outline", "", typography.outline),
          choiceRow("type-transition", "Transition", "", [
            { value: "slide", label: "Slide" },
            { value: "fade", label: "Fade" },
            { value: "off", label: "Off" },
          ], typography.transition),
          rangeRow("type-duration", "Duration", "", 100, 600, 10, typography.transition_ms, ms),
        ].join("")}
      </div>
    </details>
    ${
      track
        ? group(
            "This track",
            "",
            `<div class="inline-actions">
              <span class="inline-note">${escapeHtml(track.title)} — ${trackFont ? escapeHtml(trackFont) : "global font"}</span>
              <button class="button secondary small" type="button" data-track-font="set">Use current font</button>
              <button class="button ghost small" type="button" data-track-font="clear" ${trackFont ? "" : "disabled"}>Clear</button>
            </div>`,
          )
        : ""
    }
  `;
}

function anchorGrid(config: Config): string {
  return `<div class="anchor-grid" role="radiogroup" aria-label="Widget anchor">${ANCHORS.map(
    (anchor) =>
      `<button type="button" role="radio" class="anchor anchor-${anchor.replaceAll("_", "-")} ${config.layout.anchor === anchor ? "is-selected" : ""}" data-anchor="${anchor}" aria-checked="${config.layout.anchor === anchor}" aria-label="${anchor.replaceAll("_", " ")}"><span></span></button>`,
  ).join("")}</div>`;
}

function renderLayout(config: Config): string {
  return `
    ${sectionTools("layout")}
    ${group(
      "Mode",
      "",
      choiceRow(
        "layout-mode",
        "Widget mode",
        "",
        [
          { value: "card", label: "Card" },
          { value: "lyrics", label: "Lyrics" },
          { value: "island", label: "Island" },
        ],
        config.mode,
      ),
    )}
    ${group(
      "Size",
      "",
      [
        rangeRow("layout-scale", "Scale", "", 50, 200, 1, config.layout.scale, (value) => `${Math.round(value)}%`),
        rangeRow("layout-width", "Width", "", 240, 640, 1, config.layout.width, (value) => `${Math.round(value)} px`),
      ].join(""),
    )}
    ${group(
      "Position",
      "",
      `<div class="anchor-wrap">${anchorGrid(config)}</div>
      <div class="row-pair">
        ${rangeRow("layout-offset-x", "Horizontal offset", "", -2000, 2000, 1, config.layout.offset_x, (value) => `${Math.round(value)} px`)}
        ${rangeRow("layout-offset-y", "Vertical offset", "", -2000, 2000, 1, config.layout.offset_y, (value) => `${Math.round(value)} px`)}
      </div>
      ${checkRow("layout-edge-snap", "Snap to screen edges", "", config.behavior.edge_snap)}`,
    )}
  `;
}

function renderLyrics(config: Config): string {
  const track = currentTrack();
  const trackOffset = track ? config.per_song[track.key]?.offset_ms ?? 0 : 0;
  const demo = track?.source_app === "Iceberg demo";
  return `
    ${sectionTools("lyrics")}
    ${group(
      "Global sync",
      "",
      [
        rangeRow("lyrics-offset", "Offset", "", -1000, 1000, 10, config.lyrics.offset_ms, (value) => `${Math.round(value)} ms`),
        choiceRow("lyrics-unsynced", "Plain lyrics", "", [
          { value: "static", label: "Static page" },
          { value: "hide", label: "Hide" },
        ], config.lyrics.unsynced),
      ].join(""),
    )}
    ${
      track
        ? group(
            "This track",
            demo
              ? "Demo preview. Start a track in Spotify and this switches to the real SMTC session automatically."
              : "",
            `<div class="inline-actions">
              <span class="inline-note">${escapeHtml(track.title)} — ${trackOffset >= 0 ? "+" : ""}${trackOffset} ms${demo ? " · demo preview" : ""}</span>
              <button class="button secondary small" type="button" data-track-offset="-100">−100 ms</button>
              <button class="button secondary small" type="button" data-track-offset="100">+100 ms</button>
              <button class="button ghost small" type="button" data-track-offset="reset" ${trackOffset === 0 ? "disabled" : ""}>Reset</button>
            </div>`,
          )
        : ""
    }
    ${group(
      "Lyrics lookup",
      state.status.lyrics_message ?? "",
      `<div class="inline-actions">
        <span class="inline-note">${state.status.line ? escapeHtml(state.status.line) : "No current lyric line"}</span>
        <button class="button secondary small" id="retry-lyrics" type="button">Look up again</button>
        <button class="button ghost small" id="copy-lyric" type="button" ${state.status.line ? "" : "disabled"}>Copy line</button>
      </div>`,
    )}
  `;
}

function renderPlayback(config: Config): string {
  const behavior = config.behavior;
  return `
    ${sectionTools("playback")}
    ${group(
      "Music source",
      "Only the selected player is followed. Browser tabs and YouTube are ignored unless you explicitly choose Any app.",
      `${row(
        "Music app",
        "",
        `<select id="behavior-source" class="select">
          <option value="spotify" ${behavior.source === "spotify" ? "selected" : ""}>Spotify</option>
          <option value="apple_music" ${behavior.source === "apple_music" ? "selected" : ""}>Apple Music</option>
          <option value="custom" ${behavior.source === "custom" ? "selected" : ""}>Custom</option>
          <option value="any" ${behavior.source === "any" ? "selected" : ""}>Any app</option>
        </select>`,
        "behavior-source",
      )}
      ${
        behavior.source === "custom"
          ? row(
              "Custom match",
              "Windows source id or a unique part, for example MusicBee.",
              `<input id="behavior-source-custom" class="source-input" type="text" spellcheck="false" autocomplete="off" value="${escapeAttr(behavior.source_custom)}" placeholder="MusicBee">`,
              "behavior-source-custom",
            )
          : ""
      }
      ${
        behavior.source === "any"
          ? '<p class="group-note">Any app can include browser sessions. Choose Spotify or Apple Music if you do not want YouTube.</p>'
          : ""
      }`,
    )}
    ${group(
      "Widget behavior",
      "",
      [
        checkRow("behavior-progress", "Show progress bar", "", behavior.show_progress),
        checkRow("behavior-click-through", "Click-through", "", behavior.click_through),
        checkRow("behavior-startup", "Start with Windows", "", behavior.start_with_windows),
      ].join(""),
    )}
    <details class="advanced">
      <summary>Desktop fine tuning</summary>
      <div class="details-body">
        ${[
          checkRow("behavior-fullscreen", "Hide on fullscreen", "", behavior.hide_on_fullscreen),
          checkRow("behavior-reduce-motion", "Reduce motion", "", behavior.reduce_motion),
          checkRow("behavior-battery", "Battery saver", "", behavior.battery_saver),
          rangeRow("behavior-pause-hide", "Hide when paused", "", 0, 300, 5, behavior.pause_hide_seconds, (value) => (value === 0 ? "Never" : `${Math.round(value)} s`)),
        ].join("")}
      </div>
    </details>
  `;
}

function renderHotkeys(config: Config): string {
  const fields: { key: keyof Hotkeys; label: string }[] = [
    { key: "toggle", label: "Show / hide" },
    { key: "click_through", label: "Click-through" },
    { key: "offset_up", label: "Lyrics +100 ms" },
    { key: "offset_down", label: "Lyrics −100 ms" },
    { key: "mode", label: "Cycle mode" },
    { key: "peek", label: "Quick peek" },
    { key: "settings", label: "Open settings" },
  ];
  return `
    ${sectionTools("hotkeys")}
    ${group(
      "Global shortcuts",
      "Use at least one modifier. Letters, arrows, F1–F24, and comma are supported.",
      `<div class="hotkey-list">${fields
        .map(
          (field) => `
            <div class="hotkey-row">
              <div class="row-copy"><label for="hotkey-${field.key}">${field.label}</label></div>
              <input id="hotkey-${field.key}" class="hotkey-input" type="text" spellcheck="false" autocomplete="off" value="${escapeAttr(config.hotkeys[field.key])}" data-hotkey="${field.key}">
            </div>`,
        )
        .join("")}</div>`,
    )}
  `;
}

function renderAbout(): string {
  return `
    <div class="about-block">
      <div class="about-brand">Iceberg</div>
      <p class="about-copy">Version ${escapeHtml(state.version)} · GPL-3.0-only</p>
      <p class="about-copy fine-print">github.com/h4nxxlmao/iceberg</p>
      <div class="about-actions">
        <button class="button secondary" id="close-settings" type="button">Close settings</button>
      </div>
    </div>
    ${group(
      "Reset",
      "",
      `<div class="inline-actions">
        <span class="inline-note">${state.resetAllArmed ? "This replaces every setting with its default." : "Clears settings and per-track overrides."}</span>
        <button class="button ${state.resetAllArmed ? "danger" : "secondary"} small" id="reset-all" type="button">${state.resetAllArmed ? "Confirm reset" : "Reset everything"}</button>
        ${state.resetAllArmed ? '<button class="button ghost small" id="reset-cancel" type="button">Cancel</button>' : ""}
      </div>`,
    )}
  `;
}

function bindCommon(): void {
  const reset = root.querySelector<HTMLButtonElement>("[data-reset]");
  reset?.addEventListener("click", () => {
    const section = reset.dataset.reset;
    if (section) {
      void resetSection(section);
    }
  });
}

function bindRange(
  id: string,
  update: (value: number) => void,
  format: (value: number) => string,
  rerender = false,
): void {
  const input = document.getElementById(id) as HTMLInputElement | null;
  const output = document.getElementById(`${id}-out`);
  if (!input) {
    return;
  }
  const refresh = () => {
    if (output) {
      output.textContent = format(Number(input.value));
    }
  };
  input.addEventListener("input", () => {
    edit(() => update(Number(input.value)));
    refresh();
    if (rerender) {
      renderSection();
    }
  });
  refresh();
}

function bindColor(id: string, update: (value: string) => void): void {
  const input = document.getElementById(id) as HTMLInputElement | null;
  const output = document.getElementById(`${id}-out`);
  input?.addEventListener("input", () => {
    edit(() => update(input.value));
    if (output) {
      output.textContent = input.value;
    }
  });
}

function bindCheck(id: string, update: (value: boolean) => void): void {
  const input = document.getElementById(id) as HTMLInputElement | null;
  input?.addEventListener("change", () => {
    edit(() => update(input.checked));
  });
}

function bindChoice(id: string, update: (value: string) => void, rerender = false): void {
  const group = document.getElementById(id);
  if (!group) {
    return;
  }
  for (const button of group.querySelectorAll<HTMLButtonElement>("[data-value]")) {
    button.addEventListener("click", () => {
      const value = button.dataset.value;
      if (!value) {
        return;
      }
      edit(() => update(value));
      for (const other of group.querySelectorAll<HTMLButtonElement>("[data-value]")) {
        const selected = other === button;
        other.classList.toggle("is-selected", selected);
        other.setAttribute("aria-checked", String(selected));
      }
      if (rerender) {
        renderSection();
      }
    });
  }
}

function bindAppearance(config: Config): void {
  for (const button of root.querySelectorAll<HTMLButtonElement>("[data-surface]")) {
    button.addEventListener("click", () => {
      const kind = button.dataset.surface as BackgroundKind | undefined;
      if (!kind) {
        return;
      }
      edit(() => {
        config.background.kind = kind;
      });
      renderSection();
    });
  }
  for (const button of root.querySelectorAll<HTMLButtonElement>("[data-preset]")) {
    button.addEventListener("click", () => {
      const id = button.dataset.preset;
      if (id) {
        void applyPreset(id);
      }
    });
  }
  bindRange("bg-solid-opacity", (value) => { config.background.solid_opacity = value; }, pct);
  bindColor("bg-solid", (value) => { config.background.solid_color = value; });
  bindRange("bg-transparent-opacity", (value) => { config.background.transparent_opacity = value; }, pct);
  bindColor("bg-acrylic-tint", (value) => { config.background.acrylic_tint = value; });
  bindRange("bg-acrylic-strength", (value) => { config.background.acrylic_tint_strength = value; }, pct);
  bindRange("bg-acrylic-noise", (value) => { config.background.acrylic_noise = value; }, pct);
  bindColor("bg-ice-tint", (value) => { config.background.ice_tint = value; });
  bindRange("bg-ice-frost", (value) => { config.background.ice_frost = value; }, pct);
  bindRange("bg-ice-clarity", (value) => { config.background.ice_clarity = value; }, pct);
  bindRange("bg-liquid-refraction", (value) => { config.background.liquid_refraction = value; }, pct);
  bindRange("bg-liquid-blur", (value) => { config.background.liquid_blur = value; }, (value) => `${Math.round(value)} px`);
  bindRange("bg-liquid-highlight", (value) => { config.background.liquid_highlight = value; }, pct);
  bindRange("bg-liquid-bevel", (value) => { config.background.liquid_bevel = value; }, (value) => `${Math.round(value)} px`);
  bindCheck("bg-liquid-chromatic", (value) => { config.background.liquid_chromatic = value; });
  bindRange("bg-radius", (value) => { config.background.radius = Math.round(value); }, (value) => `${Math.round(value)} px`);
  bindRange("bg-padding", (value) => { config.background.padding = value; }, (value) => `${Math.round(value)} px`);
  bindRange("bg-border-width", (value) => { config.background.border_width = value; }, (value) => `${value.toFixed(1)} px`);
  bindColor("bg-border-color", (value) => { config.background.border_color = value; });
  bindRange("bg-border-opacity", (value) => { config.background.border_opacity = value; }, pct);
  bindCheck("bg-shadow", (value) => { config.background.shadow = value; });
  bindRange("bg-shadow-opacity", (value) => { config.background.shadow_opacity = value; }, pct);
  bindColor("accent-color", (value) => { config.background.accent_color = value; });
  bindCheck("accent-from-art", (value) => { config.background.accent_from_art = value; });
}

function bindTypography(config: Config): void {
  for (const button of root.querySelectorAll<HTMLButtonElement>("[data-font]")) {
    button.addEventListener("click", () => {
      const family = button.dataset.font;
      if (!family) {
        return;
      }
      edit(() => {
        config.typography.font_family = family;
      });
      renderSection();
    });
  }
  for (const button of root.querySelectorAll<HTMLButtonElement>("[data-track-font]")) {
    button.addEventListener("click", () => {
      const track = currentTrack();
      if (!track) {
        return;
      }
      edit(() => {
        if (button.dataset.trackFont === "clear") {
          setSongFont(config, track.key, null);
        } else {
          setSongFont(config, track.key, config.typography.font_family);
        }
      });
      renderSection();
    });
  }
  document.getElementById("upload-font")?.addEventListener("click", () => {
    void uploadFont();
  });
  bindRange("type-size", (value) => { config.typography.font_size = value; }, (value) => `${value.toFixed(1)} px`);
  bindRange("type-weight", (value) => { config.typography.weight = Math.round(value); }, (value) => String(Math.round(value)));
  bindRange("type-line-height", (value) => { config.typography.line_height = value; }, (value) => value.toFixed(2));
  bindRange("type-letter-spacing", (value) => { config.typography.letter_spacing = value; }, (value) => `${value.toFixed(2)} em`);
  bindCheck("type-italic", (value) => { config.typography.italic = value; });
  bindChoice("type-align", (value) => { config.typography.align = value as Align; });
  bindChoice("type-case", (value) => { config.typography.case = value as TextCase; });
  bindColor("type-active", (value) => { config.typography.active_color = value; });
  bindRange("type-inactive", (value) => { config.typography.inactive_opacity = value; }, pct);
  bindRange("type-visible", (value) => { config.typography.visible_lines = Math.round(value); }, (value) => String(Math.round(value)));
  bindCheck("type-shadow", (value) => { config.typography.shadow = value; });
  bindCheck("type-outline", (value) => { config.typography.outline = value; });
  bindChoice("type-transition", (value) => { config.typography.transition = value as Transition; });
  bindRange("type-duration", (value) => { config.typography.transition_ms = Math.round(value); }, ms);
}

function bindLayout(config: Config): void {
  bindChoice("layout-mode", (value) => {
    config.mode = value as Config["mode"];
  });
  bindRange("layout-scale", (value) => {
    config.layout.scale = Math.round(value);
    config.layout.per_monitor = {};
  }, (value) => `${Math.round(value)}%`);
  bindRange("layout-width", (value) => {
    config.layout.width = value;
    config.layout.per_monitor = {};
  }, (value) => `${Math.round(value)} px`);
  for (const button of root.querySelectorAll<HTMLButtonElement>("[data-anchor]")) {
    button.addEventListener("click", () => {
      const anchor = button.dataset.anchor as Anchor | undefined;
      if (!anchor) {
        return;
      }
      edit(() => {
        config.layout.anchor = anchor;
        config.layout.per_monitor = {};
      });
      for (const other of root.querySelectorAll<HTMLButtonElement>("[data-anchor]")) {
        const selected = other === button;
        other.classList.toggle("is-selected", selected);
        other.setAttribute("aria-checked", String(selected));
      }
    });
  }
  bindRange("layout-offset-x", (value) => {
    config.layout.offset_x = Math.round(value);
    config.layout.per_monitor = {};
  }, (value) => `${Math.round(value)} px`);
  bindRange("layout-offset-y", (value) => {
    config.layout.offset_y = Math.round(value);
    config.layout.per_monitor = {};
  }, (value) => `${Math.round(value)} px`);
  bindCheck("layout-edge-snap", (value) => {
    config.behavior.edge_snap = value;
  });
}

function bindLyrics(config: Config): void {
  bindRange("lyrics-offset", (value) => { config.lyrics.offset_ms = Math.round(value); }, (value) => `${Math.round(value)} ms`);
  bindChoice("lyrics-unsynced", (value) => { config.lyrics.unsynced = value as "static" | "hide"; });
  for (const button of root.querySelectorAll<HTMLButtonElement>("[data-track-offset]")) {
    button.addEventListener("click", () => {
      const track = currentTrack();
      if (!track) {
        return;
      }
      const raw = button.dataset.trackOffset;
      edit(() => {
        const current = config.per_song[track.key]?.offset_ms ?? 0;
        const next = raw === "reset" ? 0 : current + Number(raw);
        setSongOffset(config, track.key, next);
      });
      renderSection();
    });
  }
  document.getElementById("retry-lyrics")?.addEventListener("click", () => {
    void retryLyrics();
  });
  document.getElementById("copy-lyric")?.addEventListener("click", () => {
    void copyCurrentLine();
  });
}

async function retryLyrics(): Promise<void> {
  try {
    await invoke("retry_lyrics");
    showBanner("Looking up lyrics again.", "info");
  } catch (error) {
    showBanner(String(error));
  }
}

async function copyCurrentLine(): Promise<void> {
  const line = state.status.line?.trim();
  if (!line) {
    return;
  }
  try {
    await navigator.clipboard.writeText(line);
    showBanner("Current lyric copied.", "info");
  } catch (error) {
    showBanner(`Could not copy the lyric: ${String(error)}`);
  }
}

function bindPlayback(config: Config): void {
  const sourceSelect = document.getElementById("behavior-source") as HTMLSelectElement | null;
  const applySource = () => {
    if (!sourceSelect) {
      return;
    }
    const value = sourceSelect.value as SourceMode;
    edit(() => {
      config.behavior.source = value;
      config.behavior.preferred_apps = [];
    });
    renderSection();
  };
  sourceSelect?.addEventListener("change", applySource);
  sourceSelect?.addEventListener("input", applySource);
  const customSource = document.getElementById("behavior-source-custom") as HTMLInputElement | null;
  customSource?.addEventListener("change", () => {
    edit(() => {
      config.behavior.source_custom = customSource.value.trim();
    });
  });
  bindCheck("behavior-progress", (value) => { config.behavior.show_progress = value; });
  bindCheck("behavior-click-through", (value) => { config.behavior.click_through = value; });
  bindCheck("behavior-fullscreen", (value) => { config.behavior.hide_on_fullscreen = value; });
  bindCheck("behavior-startup", (value) => { config.behavior.start_with_windows = value; });
  bindCheck("behavior-reduce-motion", (value) => { config.behavior.reduce_motion = value; });
  bindCheck("behavior-battery", (value) => { config.behavior.battery_saver = value; });
  bindRange("behavior-pause-hide", (value) => {
    config.behavior.pause_hide_seconds = Math.round(value);
  }, (value) => (value === 0 ? "Never" : `${Math.round(value)} s`));
}

function bindHotkeys(config: Config): void {
  for (const input of root.querySelectorAll<HTMLInputElement>("[data-hotkey]")) {
    input.addEventListener("change", () => {
      const key = input.dataset.hotkey as keyof Hotkeys | undefined;
      if (!key) {
        return;
      }
      edit(() => { config.hotkeys[key] = input.value.trim(); });
    });
  }
}

function bindAbout(): void {
  document.getElementById("close-settings")?.addEventListener("click", () => {
    void getCurrentWindow().close();
  });
  const reset = document.getElementById("reset-all");
  reset?.addEventListener("click", () => {
    if (!state.resetAllArmed) {
      state.resetAllArmed = true;
      renderSection();
      return;
    }
    state.resetAllArmed = false;
    void resetSection("all");
  });
  document.getElementById("reset-cancel")?.addEventListener("click", () => {
    state.resetAllArmed = false;
    renderSection();
  });
}

function setSongOffset(config: Config, key: string, value: number): void {
  const entry = config.per_song[key] ?? { offset_ms: null, font_family: null };
  entry.offset_ms = value === 0 ? null : Math.max(-10000, Math.min(10000, Math.round(value)));
  config.per_song[key] = entry;
  cleanupSong(config, key);
}

function setSongFont(config: Config, key: string, family: string | null): void {
  const entry = config.per_song[key] ?? { offset_ms: null, font_family: null };
  entry.font_family = family && family.trim() ? family : null;
  config.per_song[key] = entry;
  cleanupSong(config, key);
}

function cleanupSong(config: Config, key: string): void {
  const entry = config.per_song[key];
  if (entry && entry.offset_ms == null && !entry.font_family) {
    delete config.per_song[key];
  }
}

function edit(mutate: () => void): void {
  if (!state.config) {
    return;
  }
  mutate();
  state.config.active_preset = null;
  for (const preset of root.querySelectorAll<HTMLElement>(".preset.is-selected")) {
    preset.classList.remove("is-selected");
    preset.setAttribute("aria-pressed", "false");
  }
  queueSave();
}

function queueSave(): void {
  setSaveState("saving");
  window.clearTimeout(saveTimer);
  saveTimer = window.setTimeout(() => {
    saveTimer = undefined;
    void flushSave();
  }, 140);
}

async function flushSave(): Promise<void> {
  if (!state.config) {
    return;
  }
  if (savePromise) {
    saveQueued = true;
    return;
  }
  setSaveState("saving");
  savePromise = invoke<Config>("save_config", { config: state.config })
    .then((saved) => {
      state.config = saved;
      setSaveState("saved");
      hideBanner();
    })
    .catch((error) => {
      setSaveState("error");
      showBanner(String(error));
    })
    .finally(() => {
      savePromise = null;
      if (saveQueued) {
        saveQueued = false;
        queueSave();
      }
    });
}

async function resetSection(section: string): Promise<void> {
  try {
    const config = await invoke<Config>("reset_section", { section });
    state.config = config;
    setSaveState("saved");
    hideBanner();
    renderSection();
  } catch (error) {
    showBanner(String(error));
  }
}

async function applyPreset(id: string): Promise<void> {
  try {
    const config = await invoke<Config>("apply_preset", { id });
    state.config = config;
    setSaveState("saved");
    hideBanner();
    renderSection();
  } catch (error) {
    showBanner(String(error));
  }
}

async function uploadFont(): Promise<void> {
  state.uploading = true;
  renderSection();
  try {
    const result = await invoke<FontUpload>("upload_font");
    state.fonts = result.fonts;
    if (state.config) {
      state.config.typography.font_family = result.family;
      state.config.active_preset = null;
    }
    setSaveState("saved");
    hideBanner();
    state.fontPreviews.clear();
    state.fontPreviewLine = "";
    state.fontPreviewFailed = false;
    renderSection();
    queueSave();
    void ensureFontPreviews();
  } catch (error) {
    showBanner(String(error));
    state.uploading = false;
    renderSection();
  }
}

async function revealWidget(): Promise<void> {
  try {
    await invoke("show_widget");
    showBanner("The widget is showing on your desktop.", "info");
  } catch (error) {
    showBanner(String(error));
  }
}

async function ensureFontPreviews(): Promise<void> {
  const line = FONT_SPECIMEN;
  if (
    fontPreviewRequest ||
    (state.fontPreviewLine === line &&
      (state.fontPreviews.size > 0 || state.fontPreviewFailed))
  ) {
    return;
  }
  fontPreviewRequest = invoke<FontPreview[]>("font_previews", { text: line })
    .then((previews) => {
      state.fontPreviews = new Map(
        previews.map((preview) => [
          preview.family,
          `data:image/png;base64,${preview.data}`,
        ]),
      );
      state.fontPreviewLine = line;
      state.fontPreviewFailed = false;
    })
    .catch(() => {
      state.fontPreviewLine = line;
      state.fontPreviewFailed = true;
    })
    .finally(() => {
      fontPreviewRequest = null;
      if (state.section === "typography") {
        updateFontPreviewTiles();
      }
    });
  await fontPreviewRequest;
}

function updateFontPreviewTiles(): void {
  for (const tile of root.querySelectorAll<HTMLButtonElement>("[data-font]")) {
    const family = tile.dataset.font;
    if (!family) {
      continue;
    }
    const holder = tile.querySelector<HTMLElement>(".font-preview");
    if (!holder) {
      continue;
    }
    const preview = state.fontPreviews.get(family);
    holder.innerHTML = preview
      ? `<img src="${preview}" alt="">`
      : `<span class="font-preview-placeholder">${state.fontPreviewFailed ? "Preview unavailable" : "Rendering specimen"}</span>`;
  }
}

function hydrateImages(): void {
  for (const image of root.querySelectorAll<HTMLImageElement>("img[data-image]")) {
    const key = image.dataset.image;
    if (!key) {
      continue;
    }
    const cached = state.images.get(key);
    if (cached) {
      image.src = cached;
      continue;
    }
    const [kind, id] = key.split(":");
    const request =
      kind === "preset"
        ? invoke<ImageData>("preset_image", { id })
        : invoke<ImageData>("surface_image", { kind: id });
    request
      .then((data) => {
        const url = `data:image/png;base64,${data.data}`;
        state.images.set(key, url);
        image.src = url;
      })
      .catch(() => {
        image.classList.add("is-missing");
        image.alt = "";
      });
  }
}

async function refreshFromWidget(): Promise<void> {
  if (pointerActive || saveTimer || savePromise) {
    return;
  }
  const baseline = JSON.stringify(state.config);
  try {
    const data = await invoke<SyncState>("refresh");
    if (
      pointerActive ||
      saveTimer ||
      savePromise ||
      JSON.stringify(state.config) !== baseline
    ) {
      return;
    }
    state.status = data.status;
    const previous = JSON.stringify(state.config);
    const next = JSON.stringify(data.config);
    state.config = data.config;
    updateStatus();
    if (previous !== next) {
      const focused =
        document.activeElement instanceof HTMLInputElement ||
        document.activeElement instanceof HTMLTextAreaElement ||
        document.activeElement instanceof HTMLSelectElement ||
        document.activeElement instanceof HTMLButtonElement;
      if (!focused) {
        renderSection();
      }
    }
  } catch {
    // The widget can be restarted; the next change or focus retries.
  }
}

window.addEventListener("focus", () => {
  void refreshFromWidget();
});

window.setInterval(() => {
  if (document.hidden || pointerActive || saveTimer || savePromise) {
    return;
  }
  void invoke<IpcStatus>("status")
    .then((status) => {
      const previousTrack = state.status.track?.key;
      state.status = status;
      updateStatus();
      if (
        previousTrack !== status.track?.key &&
        !pointerActive &&
        (state.section === "lyrics" || state.section === "typography")
      ) {
        renderSection();
      }
    })
    .catch(() => {
      // The next poll or focus refresh retries after a widget restart.
    });
}, 1000);

void start();
