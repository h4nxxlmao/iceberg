<script lang="ts">
  import { onMount, tick } from "svelte";
  import { api, events } from "$shared/api";
  import { hexToRgba, clamp, formatMs } from "$shared/format";
  import {
    DEFAULT_CONFIG,
    type Config,
    type LyricLine,
    type Lyrics,
    type Playback,
    type Snapshot,
    type Track,
  } from "$shared/types";

  let snap = $state<Snapshot | null>(null);
  let currentIndex = $state(-1);
  let hovered = $state(false);
  let expanded = $state(false);
  let faded = $state(false);
  let copied = $state(false);
  let offsetFlash = $state("");
  let titleOverflow = $state(false);
  let artistOverflow = $state(false);
  let titleShift = $state(0);
  let artistShift = $state(0);
  let progressEl = $state<HTMLDivElement | undefined>(undefined);
  let titleEl = $state<HTMLDivElement | undefined>(undefined);
  let artistEl = $state<HTMLDivElement | undefined>(undefined);

  let lineTimer: ReturnType<typeof setTimeout> | undefined;
  let progressTimer: ReturnType<typeof setTimeout> | undefined;
  let collapseTimer: ReturnType<typeof setTimeout> | undefined;
  let copyTimer: ReturnType<typeof setTimeout> | undefined;
  let offsetTimer: ReturnType<typeof setTimeout> | undefined;
  let fadeTimer: ReturnType<typeof setTimeout> | undefined;
  let unlisten: Array<() => void> = [];
  let collapseAt = 0;

  const config: Config = $derived(snap?.config ?? DEFAULT_CONFIG);
  const track: Track | null = $derived(snap?.track ?? null);
  const playback: Playback | null = $derived(snap?.playback ?? null);
  const lyrics: Lyrics | null = $derived(snap?.lyrics ?? null);
  const accent = $derived(
    config.appearance.accent_from_art && track?.accent ? track.accent : config.appearance.accent_color,
  );
  const effectiveOffset = $derived(snap?.effective_offset_ms ?? config.lyrics.offset_ms);
  const playing = $derived(playback?.status === "playing");
  const showBody = $derived(config.mode !== "island" || expanded);
  const durationLabel = $derived(formatMs(playback?.duration_ms || track?.duration_ms || 0));
  const lineCount = $derived(
    config.mode === "lyrics" ? Math.max(1, config.appearance.lyric_lines - 1) : config.appearance.lyric_lines,
  );

  const visibleLines = $derived.by(() => {
    if (!lyrics || lyrics.kind !== "synced" || lyrics.lines.length === 0) {
      return [] as Array<{ line: LyricLine; index: number }>;
    }
    const total = lyrics.lines.length;
    const count = clamp(lineCount, 1, 5);
    let start: number;
    if (currentIndex < 0) {
      start = 0;
    } else {
      const behind = Math.min(1, Math.floor(count / 2));
      start = Math.max(0, currentIndex - behind);
      if (start + count > total) start = Math.max(0, total - count);
    }
    return lyrics.lines.slice(start, start + count).map((line, i) => ({ line, index: start + i }));
  });

  const shellVars = $derived(
    [
      `--ice-bg:${hexToRgba(config.appearance.background_color, config.appearance.opacity)}`,
      `--ice-bg-solid:${config.appearance.background_color}`,
      `--ice-text:${config.appearance.text_color}`,
      `--ice-accent:${accent}`,
      `--ice-font:${config.appearance.font_family}`,
      `--ice-cjk:${config.lyrics.cjk_fallback ? '"Microsoft YaHei UI","Yu Gothic UI","Noto Sans"' : '"Segoe UI"'}`,
      `--ice-size:${config.appearance.font_size}px`,
      `--ice-weight:${config.appearance.font_weight}`,
      `--ice-radius:${config.appearance.radius}px`,
      `--ice-line:${hexToRgba(config.appearance.text_color, 0.14)}`,
      `--ice-soft:${hexToRgba(config.appearance.text_color, 0.06)}`,
      `--ice-border:${config.appearance.border ? `1px solid ${hexToRgba(config.appearance.text_color, 0.16)}` : "1px solid transparent"}`,
      `--ice-shadow:${config.appearance.shadow ? "0 10px 28px rgba(0,0,0,0.32)" : "none"}`,
      `--ice-align:${config.appearance.align}`,
    ].join(";"),
  );

  function effectivePosition(): number {
    if (!playback) return 0;
    let pos = playback.position_ms;
    if (playback.status === "playing") {
      pos += Math.max(0, Date.now() - playback.received_at_ms);
    }
    return pos + effectiveOffset;
  }

  function recompute() {
    clearTimeout(lineTimer);
    lineTimer = undefined;
    const lines = lyrics?.kind === "synced" ? lyrics.lines : null;
    if (!lines || lines.length === 0) {
      currentIndex = -1;
      return;
    }
    const pos = effectivePosition();
    let idx = -1;
    let lo = 0;
    let hi = lines.length - 1;
    while (lo <= hi) {
      const mid = (lo + hi) >> 1;
      if (lines[mid].t <= pos) {
        idx = mid;
        lo = mid + 1;
      } else {
        hi = mid - 1;
      }
    }
    currentIndex = idx;
    if (!playing) return;
    const nextIndex = idx + 1;
    if (nextIndex >= lines.length) return;
    const wait = Math.max(0, lines[nextIndex].t - pos);
    lineTimer = setTimeout(recompute, Math.min(wait, 3_600_000));
  }

  function clearLineTimer() {
    clearTimeout(lineTimer);
    lineTimer = undefined;
  }

  function progressPosition(): number {
    if (!playback) return 0;
    return (
      playback.position_ms +
      (playback.status === "playing" ? Math.max(0, Date.now() - playback.received_at_ms) : 0)
    );
  }

  function applyProgress(animated: boolean) {
    const el = progressEl;
    if (!el || !playback) return;
    const duration = playback.duration_ms;
    if (!duration || duration <= 0) {
      clearTimeout(progressTimer);
      el.style.transition = "none";
      el.style.transform = "scaleX(0)";
      return;
    }
    const position = clamp(progressPosition(), 0, duration);
    el.style.transition =
      animated && !config.appearance.reduce_motion ? "transform 60ms linear" : "none";
    el.style.transform = `scaleX(${position / duration})`;
  }

  function scheduleProgress() {
    clearTimeout(progressTimer);
    progressTimer = undefined;
    if (!playback || playback.status !== "playing" || playback.duration_ms <= 0) return;
    progressTimer = setTimeout(() => {
      applyProgress(true);
      scheduleProgress();
    }, 10_000);
  }

  function syncProgress() {
    applyProgress(false);
    scheduleProgress();
  }

  async function measureOverflow() {
    await tick();
    if (titleEl) {
      titleOverflow = titleEl.scrollWidth > titleEl.clientWidth + 2;
      titleShift = Math.max(0, titleEl.scrollWidth - titleEl.clientWidth + 12);
    }
    if (artistEl) {
      artistOverflow = artistEl.scrollWidth > artistEl.clientWidth + 2;
      artistShift = Math.max(0, artistEl.scrollWidth - artistEl.clientWidth + 12);
    }
  }

  function showOffset(delta: number, perSong: boolean) {
    offsetFlash = `${delta > 0 ? "+" : ""}${delta} ms${perSong ? " · song" : ""}`;
    clearTimeout(offsetTimer);
    offsetTimer = setTimeout(() => {
      offsetFlash = "";
    }, 1600);
  }

  async function copyLine(text: string) {
    if (!text.trim()) return;
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      try {
        await api.copyText(text);
      } catch {
        return;
      }
    }
    copied = true;
    clearTimeout(copyTimer);
    copyTimer = setTimeout(() => {
      copied = false;
    }, 1400);
  }

  function scheduleFade() {
    clearTimeout(fadeTimer);
    if (!config.behavior.fade_after_seconds || hovered) return;
    fadeTimer = setTimeout(() => {
      if (!hovered) faded = true;
    }, config.behavior.fade_after_seconds * 1000);
  }

  function wake() {
    faded = false;
    scheduleFade();
  }

  function onEnter() {
    hovered = true;
    faded = false;
    clearTimeout(collapseTimer);
    if (config.mode === "island") {
      expanded = true;
      void api.setHover(true);
    }
  }

  function onLeave() {
    hovered = false;
    if (config.mode === "island") {
      clearTimeout(collapseTimer);
      expanded = false;
      void api.setHover(false);
    }
    scheduleFade();
  }

  function expandForTrack() {
    if (config.mode !== "island") return;
    expanded = true;
    collapseAt = Date.now() + 4200;
    void api.setHover(true);
    clearTimeout(collapseTimer);
    collapseTimer = setTimeout(() => {
      if (!hovered && Date.now() >= collapseAt) {
        expanded = false;
        void api.setHover(false);
      }
    }, 4300);
  }

  function applyTrackChange(payload: { track: Track | null; playback: Playback }) {
    if (!snap) return;
    snap = {
      ...snap,
      track: payload.track,
      playback: payload.playback,
      lyrics: payload.track && payload.track.key === snap.track?.key ? snap.lyrics : null,
    };
    currentIndex = -1;
    if (payload.track) {
      expandForTrack();
      void measureOverflow();
    }
    void tick().then(syncProgress);
    recompute();
  }

  function applyPosition(payload: Playback) {
    if (!snap) return;
    snap = { ...snap, playback: payload };
    syncProgress();
    recompute();
  }

  function karaokeMs(index: number): number {
    const lines = lyrics?.kind === "synced" ? lyrics.lines : [];
    const line = lines[index];
    if (!line) return 4000;
    const next = lines[index + 1]?.t ?? line.t + 5000;
    return Math.max(400, next - line.t);
  }

  onMount(() => {
    void api.widgetReady();

    void Promise.all([
      events.trackChanged((p) => applyTrackChange(p)),
      events.positionSync((p) => applyPosition(p)),
      events.lyricsLoaded((p) => {
        if (!snap) return;
        snap = { ...snap, lyrics: p.lyrics, effective_offset_ms: p.effective_offset_ms };
        currentIndex = -1;
        recompute();
      }),
      events.configChanged((p) => {
        if (!snap) return;
        const previousMode = snap.config.mode;
        snap = { ...snap, config: p.config, effective_offset_ms: p.effective_offset_ms };
        if (p.config.mode !== previousMode) expanded = false;
        void tick().then(() => {
          measureOverflow();
          syncProgress();
        });
        recompute();
        scheduleFade();
      }),
      events.offset((p) => {
        if (!snap) return;
        snap = { ...snap, effective_offset_ms: p.offset_ms };
        showOffset(p.offset_ms, p.per_song);
        recompute();
      }),
      events.hideOverlay(() => {
        clearLineTimer();
        clearTimeout(progressTimer);
        progressTimer = undefined;
        clearTimeout(collapseTimer);
        clearTimeout(fadeTimer);
      }),
    ]).then((fns) => {
      unlisten = fns;
    });

    function onVisibility() {
      if (document.visibilityState === "hidden") {
        clearLineTimer();
        clearTimeout(progressTimer);
        progressTimer = undefined;
      } else {
        recompute();
        syncProgress();
        wake();
      }
    }

    document.addEventListener("visibilitychange", onVisibility);
    window.addEventListener("pointermove", wake, { passive: true });

    void api.snapshot().then((s) => {
      snap = s;
      void measureOverflow();
      void tick().then(syncProgress);
      recompute();
      scheduleFade();
    });

    return () => {
      clearLineTimer();
      clearTimeout(progressTimer);
      clearTimeout(collapseTimer);
      clearTimeout(copyTimer);
      clearTimeout(offsetTimer);
      clearTimeout(fadeTimer);
      unlisten.forEach((fn) => fn());
      document.removeEventListener("visibilitychange", onVisibility);
      window.removeEventListener("pointermove", wake);
    };
  });
</script>

<svelte:head>
  <meta name="color-scheme" content="dark light" />
</svelte:head>

<div
  class="widget mode-{config.mode}"
  class:expanded
  class:playing
  class:paused={playback?.status === "paused"}
  class:faded
  class:reduce-motion={config.appearance.reduce_motion}
  style={shellVars}
  onmouseenter={onEnter}
  onmouseleave={onLeave}
  onfocusin={onEnter}
  role="region"
  aria-label="Now playing"
>
  <div class="shell" data-tauri-drag-region>
    {#if track}
      <div class="top" data-tauri-drag-region>
        {#if config.appearance.show_cover}
          <div class="cover" class:empty={!track.cover} data-tauri-drag-region>
            {#if track.cover}
              <img src={track.cover} alt="" draggable="false" data-tauri-drag-region />
            {:else}
              <svg viewBox="0 0 24 24" aria-hidden="true">
                <path d="M9 18V5l10-2v13" />
                <circle cx="6" cy="18" r="3" />
                <circle cx="16" cy="16" r="3" />
              </svg>
            {/if}
          </div>
        {/if}

        <div class="meta" data-tauri-drag-region>
          {#if config.appearance.show_title}
            <div class="title-wrap" data-tauri-drag-region>
              <div class="title" bind:this={titleEl} data-tauri-drag-region>
                <span
                  data-tauri-drag-region
                  class:marquee={titleOverflow && hovered && !config.appearance.reduce_motion}
                  style={titleOverflow ? `--shift:-${titleShift}px;--dur:${Math.max(8, titleShift / 22)}s` : ""}
                >{track.title}</span>
              </div>
            </div>
          {/if}
          {#if config.appearance.show_artist}
            <div class="artist-wrap" data-tauri-drag-region>
              <div class="artist" bind:this={artistEl} data-tauri-drag-region>
                <span
                  data-tauri-drag-region
                  class:marquee={artistOverflow && hovered && !config.appearance.reduce_motion}
                  style={artistOverflow ? `--shift:-${artistShift}px;--dur:${Math.max(8, artistShift / 22)}s` : ""}
                >{track.artist || "Unknown artist"}</span>
              </div>
            </div>
          {/if}
        </div>

        <div class="status" aria-label={playback?.status ?? "stopped"} data-tauri-drag-region>
          {#if playback?.status === "paused"}
            <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M5 3h2v10H5zM9 3h2v10H9z" /></svg>
          {:else}
            <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M4 2.5 13 8l-9 5.5z" /></svg>
          {/if}
          {#if durationLabel !== "0:00"}
            <span class="time">{durationLabel}</span>
          {/if}
        </div>
      </div>

      {#if showBody}
        <div class="waterline" aria-hidden="true"></div>
        <div class="body" data-tauri-drag-region>
          {#if lyrics && lyrics.kind === "synced" && lyrics.lines.length > 0}
            <div class="lyrics" class:align-center={config.appearance.align === "center"}>
              {#each visibleLines as item (item.index)}
                <button
                  type="button"
                  class="lyric"
                  class:active={item.index === currentIndex}
                  class:past={item.index < currentIndex}
                  class:anim-slide={config.appearance.lyric_animation === "slide"}
                  class:anim-karaoke={config.appearance.lyric_animation === "karaoke"}
                  style={item.index === currentIndex && config.appearance.lyric_animation === "karaoke"
                    ? `--kara-dur:${karaokeMs(item.index)}ms`
                    : ""}
                  title="Click to copy"
                  onclick={() => copyLine(item.line.text)}
                >
                  <span>{item.line.text || " "}</span>
                  {#if config.appearance.lyric_animation === "karaoke" && item.index === currentIndex}
                    <i class="kara" aria-hidden="true"></i>
                  {/if}
                </button>
              {/each}
            </div>
          {:else if lyrics && lyrics.kind === "instrumental"}
            <div class="state"><span class="dot"></span>Instrumental</div>
          {:else if lyrics && lyrics.kind === "plain" && lyrics.plain.trim().length > 0}
            {#if config.lyrics.unsynced === "static"}
              <div class="plain">{lyrics.plain}</div>
            {:else}
              <div class="state">No synced lyrics</div>
            {/if}
          {:else if lyrics && lyrics.kind === "error"}
            <div class="state">{lyrics.message || "Lyrics unavailable"}</div>
          {:else if lyrics && lyrics.kind === "none"}
            <div class="state">{lyrics.message || "No lyrics found"}</div>
          {:else if track}
            <div class="state"><span class="state-mark" aria-hidden="true"></span>Looking up lyrics</div>
          {/if}
        </div>

        {#if config.appearance.show_progress && playback && playback.duration_ms > 0}
          <div class="progress" aria-hidden="true">
            <div class="bar" bind:this={progressEl}></div>
          </div>
        {/if}
      {/if}
    {:else}
      <div class="empty" data-tauri-drag-region>
        <span class="state-mark" aria-hidden="true"></span>
        <div data-tauri-drag-region>
          <div class="empty-title" data-tauri-drag-region>Nothing playing</div>
          <div class="empty-sub" data-tauri-drag-region>Start a song in any player</div>
        </div>
      </div>
    {/if}

    {#if copied}
      <div class="toast" role="status">Copied</div>
    {/if}
    {#if offsetFlash}
      <div class="toast offset" role="status">{offsetFlash}</div>
    {/if}
  </div>
</div>
