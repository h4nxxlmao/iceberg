<script lang="ts">
  import { onMount } from "svelte";
  import { api, events } from "$shared/api";
  import { fuzzyTime } from "$shared/format";
  import {
    DEFAULT_CONFIG,
    FONT_CHOICES,
    THEME_NAMES,
    type Config,
    type Diagnostics,
    type SearchResult,
    type Snapshot,
    type Track,
  } from "$shared/types";

  type Tab = "general" | "appearance" | "lyrics" | "behavior" | "hotkeys" | "advanced";

  let snapshot = $state<Snapshot | null>(null);
  let config = $state<Config>(structuredClone(DEFAULT_CONFIG));
  let tab = $state<Tab>("general");
  let status = $state("");
  let statusTimer: ReturnType<typeof setTimeout> | undefined;
  let saveTimer: ReturnType<typeof setTimeout> | undefined;
  let diagTimer: ReturnType<typeof setInterval> | undefined;
  let unlisten: Array<() => void> = [];
  let ready = false;
  let saving = false;
  let pendingSave = false;

  let diagnostics = $state<Diagnostics | null>(null);
  let searchQuery = $state("");
  let searchResults = $state<SearchResult[]>([]);
  let searching = $state(false);
  let lyricMessage = $state("");
  let preferredText = $state("");
  let ignoredText = $state("");
  let effectiveOffset = $state(0);
  let currentTrack = $state<Track | null>(null);
  let loading = $state(true);
  let loadError = $state("");
  let diagError = $state("");

  const tabs: Array<{ id: Tab; label: string }> = [
    { id: "general", label: "General" },
    { id: "appearance", label: "Appearance" },
    { id: "lyrics", label: "Lyrics" },
    { id: "behavior", label: "Behavior" },
    { id: "hotkeys", label: "Hotkeys" },
    { id: "advanced", label: "Advanced" },
  ];

  function flash(message: string) {
    status = message;
    clearTimeout(statusTimer);
    statusTimer = setTimeout(() => {
      status = "";
    }, 2600);
  }

  function queueSave() {
    if (!ready) return;
    clearTimeout(saveTimer);
    saveTimer = setTimeout(save, 150);
  }

  async function save() {
    if (saving) {
      pendingSave = true;
      return;
    }
    saving = true;
    try {
      await api.setConfig(JSON.parse(JSON.stringify(config)) as Config);
    } catch (error) {
      flash(`Could not save: ${String(error)}`);
    } finally {
      saving = false;
      if (pendingSave) {
        pendingSave = false;
        void save();
      }
    }
  }

  function setMode(mode: string) {
    config.mode = mode as Config["mode"];
    queueSave();
  }

  async function chooseTheme(id: string) {
    clearTimeout(saveTimer);
    pendingSave = false;
    try {
      const next = await api.applyTheme(id);
      config = next;
      flash(`Theme: ${THEME_NAMES.find((t) => t.id === id)?.label ?? id}`);
      if (tab === "advanced") refreshDiag();
    } catch (error) {
      flash(`Theme failed: ${String(error)}`);
    }
  }

  function resetAll() {
    if (!window.confirm("Reset every Iceberg setting to defaults?")) return;
    clearTimeout(saveTimer);
    pendingSave = false;
    void api.resetConfig().then((next) => {
      config = next;
      effectiveOffset = next.lyrics.offset_ms;
      preferredText = "";
      ignoredText = "";
      flash("Settings reset");
    });
  }

  function setTab(next: Tab) {
    tab = next;
    clearInterval(diagTimer);
    if (next === "advanced") {
      refreshDiag();
      diagTimer = setInterval(refreshDiag, 3000);
    }
  }

  async function refreshDiag() {
    try {
      diagnostics = await api.diagnostics();
      diagError = "";
    } catch (error) {
      diagnostics = null;
      diagError = String(error);
    }
  }

  async function loadSnapshot() {
    loading = true;
    loadError = "";
    try {
      const s = await api.snapshot();
      snapshot = s;
      config = structuredClone(s.config);
      effectiveOffset = s.effective_offset_ms;
      currentTrack = s.track;
      preferredText = s.config.behavior.preferred_apps.join(", ");
      ignoredText = s.config.behavior.ignored_apps.join(", ");
      searchQuery = s.track ? `${s.track.title} ${s.track.artist}`.trim() : "";
      ready = true;
    } catch (error) {
      loadError = String(error);
      ready = false;
    } finally {
      loading = false;
    }
  }

  function applyApps() {
    config.behavior.preferred_apps = preferredText
      .split(",")
      .map((s) => s.trim())
      .filter(Boolean);
    config.behavior.ignored_apps = ignoredText
      .split(",")
      .map((s) => s.trim())
      .filter(Boolean);
    queueSave();
  }

  async function search() {
    if (!searchQuery.trim()) return;
    searching = true;
    lyricMessage = "";
    try {
      searchResults = await api.searchLyrics(searchQuery.trim());
      if (searchResults.length === 0) lyricMessage = "No matches on LRCLIB.";
    } catch (error) {
      lyricMessage = `Search failed: ${String(error)}`;
    } finally {
      searching = false;
    }
  }

  async function chooseResult(id: number) {
    try {
      await api.applyLyricsChoice(id);
      lyricMessage = "Lyrics replaced for this song.";
      searchResults = [];
    } catch (error) {
      lyricMessage = `Could not apply: ${String(error)}`;
    }
  }

  async function importLrc() {
    try {
      lyricMessage = await api.importLrc();
      refreshDiag();
    } catch (error) {
      lyricMessage = `Import failed: ${String(error)}`;
    }
  }

  async function adjust(delta: number) {
    try {
      effectiveOffset = await api.adjustOffset(delta);
    } catch (error) {
      flash(`Offset failed: ${String(error)}`);
    }
  }

  async function resetSongOffset() {
    try {
      effectiveOffset = await api.resetSongOffset();
      flash("Song offset cleared");
    } catch (error) {
      flash(`Offset failed: ${String(error)}`);
    }
  }

  async function clearCache() {
    try {
      const freed = await api.clearCache();
      flash(`Cleared ${fmtBytes(freed)}`);
      refreshDiag();
    } catch (error) {
      flash(`Clear failed: ${String(error)}`);
    }
  }

  async function exportTheme() {
    try {
      flash(await api.exportTheme());
    } catch (error) {
      flash(`Export failed: ${String(error)}`);
    }
  }

  async function importTheme() {
    clearTimeout(saveTimer);
    pendingSave = false;
    try {
      const name = await api.importTheme();
      flash(`Imported theme: ${name}`);
      const next = await api.snapshot();
      snapshot = next;
      config = next.config;
      effectiveOffset = next.effective_offset_ms;
    } catch (error) {
      flash(`Import failed: ${String(error)}`);
    }
  }

  async function openFolder() {
    try {
      await api.openConfigFolder();
    } catch (error) {
      flash(String(error));
    }
  }

  function fmtBytes(bytes: number): string {
    if (!bytes) return "0 B";
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }

  function fmtMb(bytes: number): string {
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }

  function updateEffective(value: number) {
    effectiveOffset = value;
  }

  onMount(() => {
    void loadSnapshot();

    void Promise.all([
      events.configChanged((p) => {
        config = p.config;
        effectiveOffset = p.effective_offset_ms;
        preferredText = p.config.behavior.preferred_apps.join(", ");
        ignoredText = p.config.behavior.ignored_apps.join(", ");
      }),
      events.trackChanged((p) => {
        currentTrack = p.track;
        if (p.track && !searchQuery) searchQuery = `${p.track.title} ${p.track.artist}`.trim();
      }),
      events.offset((p) => {
        updateEffective(p.offset_ms);
      }),
    ])
      .then((fns) => {
        unlisten = fns;
      })
      .catch((error) => {
        flash(`Live updates unavailable: ${String(error)}`);
      });

    return () => {
      clearTimeout(statusTimer);
      clearTimeout(saveTimer);
      clearInterval(diagTimer);
      unlisten.forEach((fn) => fn());
    };
  });

  function closeSettings() {
    void api.closeSettings();
  }
</script>

<svelte:head>
  <title>Iceberg Settings</title>
  <meta name="color-scheme" content="dark light" />
</svelte:head>

<div class="settings">
  <aside class="sidebar">
    <div class="brand">
      <div class="brand-name">Iceberg</div>
      <div class="brand-sub">Settings</div>
    </div>

    <nav aria-label="Settings sections">
      {#each tabs as item, index}
        <button
          type="button"
          class="nav-item"
          class:active={tab === item.id}
          aria-current={tab === item.id ? "page" : undefined}
          onclick={() => setTab(item.id)}
        >
          <span class="nav-no">{String(index + 1).padStart(2, "0")}</span>
          <span class="nav-label">{item.label}</span>
        </button>
      {/each}
    </nav>

    <div class="sidebar-foot">
      <div class="now" title={currentTrack ? `${currentTrack.title} by ${currentTrack.artist}` : "Nothing playing"}>
        <span class="now-dot" class:live={!!currentTrack}></span>
        <span>{currentTrack ? currentTrack.title : "Nothing playing"}</span>
      </div>
      <button type="button" class="link" onclick={openFolder}>Config folder</button>
      <button type="button" class="link" onclick={closeSettings}>Close settings</button>
    </div>
  </aside>

  <main class="content">
    {#if loading}
      <div class="load-state" role="status">Loading settings...</div>
    {:else if loadError}
      <div class="load-state error" role="alert">
        Settings could not load.
        <div class="hint strong">{loadError}</div>
        <div class="btn-row">
          <button type="button" class="btn ghost" onclick={() => void loadSnapshot()}>Try again</button>
        </div>
      </div>
    {:else}
      {#if tab === "general"}
      <section>
        <h1>General</h1>
        <p class="lead">Iceberg lives in the tray until something starts playing. It never asks you to log in.</p>

        <div class="group">
          <h2>Theme</h2>
          <div class="theme-grid">
            {#each THEME_NAMES as theme}
              <button
                type="button"
                class="theme"
                class:selected={config.theme === theme.id}
                aria-pressed={config.theme === theme.id}
                onclick={() => chooseTheme(theme.id)}
              >
                <span class="swatch theme-{theme.id}" aria-hidden="true"><i></i></span>
                <span class="theme-label">{theme.label}</span>
                <span class="theme-state" aria-hidden="true"></span>
              </button>
            {/each}
          </div>
          <p class="hint">Themes update the widget immediately. Every field below can still be tweaked.</p>
        </div>

        <div class="group">
          <h2>Window</h2>
          <div class="row">
            <div>
              <div class="label">Mode</div>
              <div class="desc">How much of the widget is visible.</div>
            </div>
            <div class="segmented">
              {#each [["island", "Island"], ["card", "Card"], ["lyrics", "Lyrics only"]] as [id, label]}
                <button type="button" class="seg" class:on={config.mode === id} aria-pressed={config.mode === id} onclick={() => setMode(id)}>{label}</button>
              {/each}
            </div>
          </div>

          <label class="row">
            <div>
              <div class="label">Start with Windows</div>
              <div class="desc">Launch minimized to the tray when you sign in.</div>
            </div>
            <span class="switch"><input type="checkbox" bind:checked={config.behavior.start_with_windows} onchange={queueSave} /><i></i></span>
          </label>

          <label class="row">
            <div>
              <div class="label">Battery saver preset</div>
              <div class="desc">No native blur, fade lyrics, shorter idle timeouts.</div>
            </div>
            <span class="switch"><input type="checkbox" bind:checked={config.behavior.battery_saver} onchange={queueSave} /><i></i></span>
          </label>

          <label class="row">
            <div>
              <div class="label">Reduce motion</div>
              <div class="desc">Turns off marquee and lyric transitions.</div>
            </div>
            <span class="switch"><input type="checkbox" bind:checked={config.appearance.reduce_motion} onchange={queueSave} /><i></i></span>
          </label>
        </div>
      </section>
    {/if}

    {#if tab === "appearance"}
      <section>
        <h1>Appearance</h1>
        <p class="lead">The widget is a translucent instrument. Acrylic and mica are optional, and there is no CSS blur. Every change previews live.</p>

        <div class="group">
          <h2>Surface</h2>
          <div class="row">
            <div><div class="label">Window effect</div><div class="desc">Off keeps memory lowest. Acrylic and Mica add a few MB of GPU memory.</div></div>
            <div class="segmented">
              {#each [["acrylic", "Acrylic"], ["mica", "Mica"], ["none", "None"]] as [id, label]}
                <button type="button" class="seg" class:on={config.appearance.effect === id} aria-pressed={config.appearance.effect === id} onclick={() => { config.appearance.effect = id as Config["appearance"]["effect"]; queueSave(); }}>{label}</button>
              {/each}
            </div>
          </div>

          <label class="slider-row">
            <div><div class="label">Background opacity</div><div class="desc">{Math.round(config.appearance.opacity * 100)}%{config.appearance.opacity < 0.6 ? " · low contrast" : ""}</div></div>
            <input type="range" min="0.15" max="0.98" step="0.01" bind:value={config.appearance.opacity} oninput={queueSave} />
          </label>

          <label class="slider-row">
            <div><div class="label">Corner radius</div><div class="desc">{config.appearance.radius}px</div></div>
            <input type="range" min="0" max="32" step="1" bind:value={config.appearance.radius} oninput={queueSave} />
          </label>

          <div class="row">
            <div><div class="label">Border</div><div class="desc">Thin light edge.</div></div>
            <span class="switch"><input type="checkbox" bind:checked={config.appearance.border} onchange={queueSave} /><i></i></span>
          </div>
          <div class="row">
            <div><div class="label">Drop shadow</div><div class="desc">Opt-in; costs a little GPU.</div></div>
            <span class="switch"><input type="checkbox" bind:checked={config.appearance.shadow} onchange={queueSave} /><i></i></span>
          </div>
        </div>

        <div class="group">
          <h2>Colors</h2>
          <div class="color-grid">
            <label class="color-row"><span>Text</span><input type="color" bind:value={config.appearance.text_color} oninput={queueSave} /><code>{config.appearance.text_color}</code></label>
            <label class="color-row"><span>Accent</span><input type="color" bind:value={config.appearance.accent_color} oninput={queueSave} /><code>{config.appearance.accent_color}</code></label>
            <label class="color-row"><span>Background</span><input type="color" bind:value={config.appearance.background_color} oninput={queueSave} /><code>{config.appearance.background_color}</code></label>
          </div>
          <label class="row">
            <div><div class="label">Accent from album art</div><div class="desc">Uses a muted dominant color from the cover.</div></div>
            <span class="switch"><input type="checkbox" bind:checked={config.appearance.accent_from_art} onchange={queueSave} /><i></i></span>
          </label>
        </div>

        <div class="group">
          <h2>Type</h2>
          <div class="row">
            <div><div class="label">Lyrics and body</div><div class="desc">System fonts only. The widget title stays Bahnschrift.</div></div>
            <select bind:value={config.appearance.font_family} onchange={queueSave}>
              {#each FONT_CHOICES as font}
                <option value={font.value}>{font.label}</option>
              {/each}
            </select>
          </div>
          <label class="slider-row">
            <div><div class="label">Size</div><div class="desc">{config.appearance.font_size}px</div></div>
            <input type="range" min="11" max="22" step="1" bind:value={config.appearance.font_size} oninput={queueSave} />
          </label>
          <label class="slider-row">
            <div><div class="label">Weight</div><div class="desc">{config.appearance.font_weight}</div></div>
            <input type="range" min="300" max="700" step="100" bind:value={config.appearance.font_weight} oninput={queueSave} />
          </label>
        </div>

        <div class="group">
          <h2>Lyrics display</h2>
          <div class="row">
            <div><div class="label">Animation</div><div class="desc">Transform and opacity only.</div></div>
            <div class="segmented">
              {#each [["fade", "Fade"], ["slide", "Slide"], ["karaoke", "Karaoke"]] as [id, label]}
                <button type="button" class="seg" class:on={config.appearance.lyric_animation === id} aria-pressed={config.appearance.lyric_animation === id} onclick={() => { config.appearance.lyric_animation = id as Config["appearance"]["lyric_animation"]; queueSave(); }}>{label}</button>
              {/each}
            </div>
          </div>
          <label class="slider-row">
            <div><div class="label">Lines shown</div><div class="desc">3 to 5 nodes are rendered.</div></div>
            <input type="range" min="3" max="5" step="1" bind:value={config.appearance.lyric_lines} oninput={queueSave} />
          </label>
          <div class="row">
            <div><div class="label">Alignment</div><div class="desc">Applies to lyric lines and title.</div></div>
            <div class="segmented">
              <button type="button" class="seg" class:on={config.appearance.align === "left"} aria-pressed={config.appearance.align === "left"} onclick={() => { config.appearance.align = "left"; queueSave(); }}>Left</button>
              <button type="button" class="seg" class:on={config.appearance.align === "center"} aria-pressed={config.appearance.align === "center"} onclick={() => { config.appearance.align = "center"; queueSave(); }}>Center</button>
            </div>
          </div>
          <div class="row">
            <div><div class="label">Show cover</div><div class="desc">Small cached thumbnail.</div></div>
            <span class="switch"><input type="checkbox" bind:checked={config.appearance.show_cover} onchange={queueSave} /><i></i></span>
          </div>
          <div class="row">
            <div><div class="label">Show progress</div><div class="desc">Compositor-driven bar.</div></div>
            <span class="switch"><input type="checkbox" bind:checked={config.appearance.show_progress} onchange={queueSave} /><i></i></span>
          </div>
          <div class="row">
            <div><div class="label">Show title</div><div class="desc">Marquee only when text overflows.</div></div>
            <span class="switch"><input type="checkbox" bind:checked={config.appearance.show_title} onchange={queueSave} /><i></i></span>
          </div>
          <div class="row">
            <div><div class="label">Show artist</div><div class="desc">Hide for a cleaner look.</div></div>
            <span class="switch"><input type="checkbox" bind:checked={config.appearance.show_artist} onchange={queueSave} /><i></i></span>
          </div>
        </div>
      </section>
    {/if}

    {#if tab === "lyrics"}
      <section>
        <h1>Lyrics</h1>
        <p class="lead">Lyrics come from LRCLIB. No account, no API key, no lyrics sent anywhere else.</p>

        <div class="group">
          <h2>Sync</h2>
          <label class="slider-row">
            <div><div class="label">Global offset</div><div class="desc">{(config.lyrics.offset_ms / 1000).toFixed(1)}s · positive shows lyrics earlier</div></div>
            <input type="range" min="-3000" max="3000" step="50" bind:value={config.lyrics.offset_ms} oninput={queueSave} />
          </label>
          <div class="row">
            <div><div class="label">This song</div><div class="desc">Saved automatically per track. Effective offset {effectiveOffset} ms.</div></div>
            <div class="btn-row">
              <button type="button" class="btn" onclick={() => adjust(-100)}>−100 ms</button>
              <button type="button" class="btn" onclick={() => adjust(100)}>+100 ms</button>
              <button type="button" class="btn ghost" onclick={resetSongOffset}>Reset</button>
            </div>
          </div>
          <div class="row">
            <div><div class="label">Unsynced lyrics</div><div class="desc">What to show when only plain text exists.</div></div>
            <div class="segmented">
              <button type="button" class="seg" class:on={config.lyrics.unsynced === "static"} aria-pressed={config.lyrics.unsynced === "static"} onclick={() => { config.lyrics.unsynced = "static"; queueSave(); }}>Show text</button>
              <button type="button" class="seg" class:on={config.lyrics.unsynced === "hide"} aria-pressed={config.lyrics.unsynced === "hide"} onclick={() => { config.lyrics.unsynced = "hide"; queueSave(); }}>Hide</button>
            </div>
          </div>
          <div class="row">
            <div><div class="label">CJK and RTL fallback</div><div class="desc">Falls back to a system font with better glyph coverage.</div></div>
            <span class="switch"><input type="checkbox" bind:checked={config.lyrics.cjk_fallback} onchange={queueSave} /><i></i></span>
          </div>
        </div>

        <div class="group">
          <h2>Wrong lyrics?</h2>
          <p class="hint">Search LRCLIB and replace the lyrics for the current song.</p>
          <div class="search-row">
            <input type="text" bind:value={searchQuery} placeholder="Song and artist" onkeydown={(e) => e.key === "Enter" && search()} />
            <button type="button" class="btn" onclick={search} disabled={searching}>{searching ? "Searching" : "Search"}</button>
          </div>
          {#if lyricMessage}<p class="hint strong">{lyricMessage}</p>{/if}
          {#if searchResults.length > 0}
            <div class="results">
              {#each searchResults as result}
                <div class="result">
                  <div class="result-main">
                    <div class="result-title">{result.track_name}</div>
                    <div class="result-sub">{result.artist_name}{result.album_name ? ` · ${result.album_name}` : ""} · {fuzzyTime(result.duration)}</div>
                  </div>
                  <div class="result-tags">
                    {#if result.instrumental}<span class="tag">Instrumental</span>{/if}
                    {#if result.has_synced}<span class="tag accent">Synced</span>{/if}
                    {#if !result.has_synced && result.has_plain}<span class="tag">Plain</span>{/if}
                  </div>
                  <button type="button" class="btn small" onclick={() => chooseResult(result.id)}>Use</button>
                </div>
              {/each}
            </div>
          {/if}
          <div class="btn-row">
            <button type="button" class="btn ghost" onclick={importLrc}>Import .lrc file</button>
          </div>
        </div>
      </section>
    {/if}

    {#if tab === "behavior"}
      <section>
        <h1>Behavior</h1>
        <p class="lead">Iceberg destroys its webview when it is not needed. These timers control that.</p>

        <div class="group">
          <h2>Visibility</h2>
          <label class="slider-row">
            <div><div class="label">Destroy widget after</div><div class="desc">{config.behavior.idle_destroy_minutes} min without playback</div></div>
            <input type="range" min="1" max="30" step="1" bind:value={config.behavior.idle_destroy_minutes} oninput={queueSave} />
          </label>
          <label class="slider-row">
            <div><div class="label">Auto-hide when paused</div><div class="desc">{config.behavior.auto_hide_paused_seconds}s · 0 hides immediately</div></div>
            <input type="range" min="0" max="120" step="5" bind:value={config.behavior.auto_hide_paused_seconds} oninput={queueSave} />
          </label>
          <div class="row">
            <div><div class="label">Hide over fullscreen apps</div><div class="desc">Games and fullscreen video get the whole screen.</div></div>
            <span class="switch"><input type="checkbox" bind:checked={config.behavior.hide_on_fullscreen} onchange={queueSave} /><i></i></span>
          </div>
          <div class="row">
            <div><div class="label">Click-through</div><div class="desc">Widget ignores the mouse until you toggle it back.</div></div>
            <span class="switch"><input type="checkbox" bind:checked={config.behavior.click_through} onchange={queueSave} /><i></i></span>
          </div>
          <div class="row">
            <div><div class="label">Snap to edges</div><div class="desc">Snaps to screen edges and center after dragging.</div></div>
            <span class="switch"><input type="checkbox" bind:checked={config.behavior.edge_snap} onchange={queueSave} /><i></i></span>
          </div>
          <label class="slider-row">
            <div><div class="label">Idle fade</div><div class="desc">{config.behavior.fade_after_seconds === 0 ? "Off" : `${config.behavior.fade_after_seconds}s without interaction`}</div></div>
            <input type="range" min="0" max="30" step="1" bind:value={config.behavior.fade_after_seconds} oninput={queueSave} />
          </label>
          <label class="slider-row">
            <div><div class="label">Quick peek</div><div class="desc">{config.behavior.quick_peek_seconds}s on screen</div></div>
            <input type="range" min="2" max="15" step="1" bind:value={config.behavior.quick_peek_seconds} oninput={queueSave} />
          </label>
        </div>

        <div class="group">
          <h2>Players</h2>
          <div class="field">
            <label for="preferred">Only use these apps</label>
            <input id="preferred" type="text" bind:value={preferredText} placeholder="spotify.exe, AppleMusic.exe" onblur={applyApps} />
            <p class="hint">Comma-separated source app names. Leave empty to follow every player.</p>
          </div>
          <div class="field">
            <label for="ignored">Ignore these apps</label>
            <input id="ignored" type="text" bind:value={ignoredText} placeholder="chrome.exe" onblur={applyApps} />
            <p class="hint">Useful if a browser tab keeps claiming the session. Empty by default.</p>
          </div>
        </div>
      </section>
    {/if}

    {#if tab === "hotkeys"}
      <section>
        <h1>Hotkeys</h1>
        <p class="lead">Global shortcuts work while Iceberg is in the tray. Use names like <code>Ctrl+Alt+KeyI</code>.</p>

        <div class="group">
          <h2>Shortcuts</h2>
          {#each [["toggle", "Show or hide widget"], ["click_through", "Toggle click-through"], ["offset_up", "Lyrics +100 ms"], ["offset_down", "Lyrics −100 ms"], ["mode", "Cycle widget mode"], ["peek", "Quick peek"], ["settings", "Open settings"]] as [key, label]}
            <div class="row">
              <div><div class="label">{label}</div><div class="desc">{config.hotkeys[key as keyof Config["hotkeys"]] || "Unassigned"}</div></div>
              <input class="hotkey" type="text" bind:value={config.hotkeys[key as keyof Config["hotkeys"]]} onchange={queueSave} />
            </div>
          {/each}
          <div class="btn-row">
            <button type="button" class="btn ghost" onclick={() => { config.hotkeys = structuredClone(DEFAULT_CONFIG.hotkeys); queueSave(); flash("Hotkeys restored"); }}>Restore defaults</button>
          </div>
        </div>
      </section>
    {/if}

    {#if tab === "advanced"}
      <section>
        <h1>Advanced</h1>
        <p class="lead">Diagnostics measure the whole process tree, including every WebView2 child.</p>

        <div class="group">
          <h2>Live diagnostics</h2>
          {#if diagnostics}
            <div class="readout">
              <div class="readout-main">
                <span class="readout-label">Private memory</span>
                <span class="readout-value">{fmtMb(diagnostics.private_bytes)}</span>
              </div>
              <dl class="ledger">
                <div class="ledger-row"><dt>Working set</dt><dd>{fmtMb(diagnostics.ram_bytes)}</dd></div>
                <div class="ledger-row"><dt>Processes</dt><dd>{diagnostics.process_count}</dd></div>
                <div class="ledger-row"><dt>WebView2 data</dt><dd>{fmtMb(diagnostics.webview_cache_bytes)}</dd></div>
                <div class="ledger-row"><dt>Lyrics cache</dt><dd>{fmtBytes(diagnostics.lyrics_cache_bytes)}</dd></div>
                <div class="ledger-row"><dt>SMTC source</dt><dd>{diagnostics.active_source || "none"}</dd></div>
                <div class="ledger-row"><dt>Current track</dt><dd>{diagnostics.active_title || "none"}</dd></div>
              </dl>
            </div>
            <p class="hint">{diagnostics.cpu_note}</p>
          {:else if diagError}
            <p class="hint strong">Diagnostics unavailable: {diagError}</p>
            <div class="btn-row">
              <button type="button" class="btn ghost" onclick={refreshDiag}>Try again</button>
            </div>
          {:else}
            <p class="hint">Reading diagnostics...</p>
          {/if}
          <div class="btn-row">
            <button type="button" class="btn ghost" onclick={refreshDiag}>Refresh</button>
            <button type="button" class="btn ghost" onclick={clearCache}>Clear caches</button>
            <button type="button" class="btn ghost" onclick={openFolder}>Open config folder</button>
          </div>
        </div>

        <div class="group">
          <h2>Themes</h2>
          <p class="hint">Export the current appearance as one JSON file, or import a theme someone shared.</p>
          <div class="btn-row">
            <button type="button" class="btn ghost" onclick={exportTheme}>Export theme</button>
            <button type="button" class="btn ghost" onclick={importTheme}>Import theme</button>
          </div>
        </div>

        <div class="group">
          <h2>Danger zone</h2>
          <div class="row">
            <div><div class="label">Reset everything</div><div class="desc">Config, per-song offsets, and theme go back to defaults.</div></div>
            <button type="button" class="btn danger" onclick={resetAll}>Reset</button>
          </div>
        </div>

        <div class="about">
          <div class="about-wordmark">Iceberg <span class="about-version">{snapshot?.app_version ?? ""}</span></div>
          <p>MIT licensed. No telemetry. The only network calls go to lrclib.net.</p>
          <p>Source: github.com/h4nxxlmao/iceberg</p>
          <p>Config: <code>{diagnostics?.config_path ?? ""}</code></p>
          <p>WebView2 data: <code>{diagnostics?.webview_path ?? ""}</code></p>
        </div>
      </section>
      {/if}
    {/if}

    {#if status}
      <div class="save-toast" role="status">{status}</div>
    {/if}
  </main>
</div>
