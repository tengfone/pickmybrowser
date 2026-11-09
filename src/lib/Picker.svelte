<script lang="ts">
  import { onMount, tick } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { api, applyTheme, splitUrl, PROFILE_KEYS, modKey, type Browser, type Pending, type Snapshot } from "./api";
  import Glyph from "./Glyph.svelte";
  import { loadIcons } from "./icons.svelte";

  /** Number of browsers reachable via the 1–9 hotkeys; above this, typing filters the list. */
  const HOTKEY_COUNT = 9;

  let snapshot = $state<Snapshot | null>(null);
  let pending = $state<Pending | null>(null);
  let selectedIndex = $state(0);
  let expandedId = $state<string | null>(null);
  let selectedProfile = $state(-1);
  let privateHeld = $state(false);
  let filter = $state("");
  let mode = $state<"list" | "rule">("list");
  let countdownProgress = $state(0);
  let error = $state("");
  let copied = $state(false);
  let card: HTMLElement;
  let countdownFrame = 0;

  const enabledBrowsers = $derived((snapshot?.browsers ?? []).filter((b) => b.enabled));
  const filterEnabled = $derived(enabledBrowsers.length > HOTKEY_COUNT);
  const visibleBrowsers = $derived(
    filter ? enabledBrowsers.filter((b) => b.name.toLowerCase().includes(filter.toLowerCase())) : enabledBrowsers,
  );
  const urlParts = $derived(pending ? splitUrl(pending.url) : null);
  const matchedRule = $derived(pending?.rule ?? null);
  const ruleBrowser = $derived(matchedRule ? enabledBrowsers.find((b) => b.id === matchedRule.browserId) : undefined);
  const ruleProfile = $derived(
    matchedRule?.profileId ? ruleBrowser?.profiles.find((pr) => pr.id === matchedRule.profileId) : undefined,
  );
  const os = $derived(snapshot?.os ?? "windows");
  const anyProfiles = $derived(visibleBrowsers.some((b) => b.profiles.length));

  const ruleTitle = $derived.by(() => {
    if (!ruleBrowser) return "";
    return ruleProfile ? `${ruleBrowser.name} · ${ruleProfile.name}` : ruleBrowser.name;
  });
  const countdownLabel = $derived(
    pending?.countdownMs && !pending.lookalike ? ` · ${(pending.countdownMs / 1000).toFixed(1)} s` : "",
  );

  function privateModeName(b: Browser) {
    if (b.engine !== "chromium") return "Private";
    if (b.family === "edge") return "InPrivate";
    if (b.family === "chrome") return "Incognito";
    return "Private";
  }

  function resetView() {
    selectedIndex = 0;
    expandedId = null;
    selectedProfile = -1;
    filter = "";
    error = "";
    copied = false;
    privateHeld = false;
  }

  async function show(next: Pending) {
    cancelAnimationFrame(countdownFrame);
    snapshot = await api.snapshot();
    applyTheme(snapshot.settings.theme);
    loadIcons();
    pending = next;
    resetView();
    mode = next.rule && ruleBrowser ? "rule" : "list";
    await tick();
    await api.present(card.offsetWidth, card.offsetHeight);
    card.focus();
    if (mode === "rule") startCountdown();
  }

  function startCountdown() {
    const duration = pending?.countdownMs ?? 0;
    if (pending?.lookalike || duration <= 0) {
      countdownProgress = 0;
      return;
    }
    const start = performance.now();
    const step = (now: number) => {
      countdownProgress = Math.min(1, (now - start) / duration);
      if (mode !== "rule") return;
      if (countdownProgress >= 1) openRule();
      else countdownFrame = requestAnimationFrame(step);
    };
    countdownFrame = requestAnimationFrame(step);
  }

  /** Let the window follow the card size after layout changes, without moving it. */
  async function resize() {
    await tick();
    if (card) await api.resize(card.offsetWidth, card.offsetHeight);
  }

  async function open(b: Browser, profileId: string | null = null, incognito = privateHeld) {
    if (!pending) return;
    cancelAnimationFrame(countdownFrame);
    try {
      await api.open(pending.url, b.id, profileId, incognito);
      pending = null;
    } catch (e) {
      error = String(e);
      resize();
    }
  }

  function openRule() {
    if (ruleBrowser) open(ruleBrowser, matchedRule?.profileId ?? null, false);
  }

  function showList() {
    cancelAnimationFrame(countdownFrame);
    mode = "list";
    selectedIndex = Math.max(0, visibleBrowsers.findIndex((b) => b.id === ruleBrowser?.id));
    resize();
  }

  function close() {
    cancelAnimationFrame(countdownFrame);
    pending = null;
    api.dismiss();
  }

  function toggleProfiles(b: Browser, expand = expandedId !== b.id) {
    expandedId = expand && b.profiles.length ? b.id : null;
    selectedProfile = expandedId ? 0 : -1;
    resize();
  }

  function selectRow(index: number) {
    selectedIndex = index;
    expandedId = null;
    selectedProfile = -1;
    resize();
  }

  async function copyUrl() {
    if (!pending) return;
    await navigator.clipboard.writeText(pending.url);
    copied = true;
    setTimeout(() => (copied = false), 1200);
  }

  function onRuleKey(e: KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      openRule();
    } else if (e.key === " " || e.key === "Escape") {
      e.preventDefault();
      showList();
    }
  }

  /** Returns true when the key was handled. */
  function onListNavKey(e: KeyboardEvent, current: Browser | undefined, expanded: Browser | null) {
    switch (e.key) {
      case "Escape":
        e.preventDefault();
        if (expanded) toggleProfiles(expanded, false);
        else if (filter) filter = "";
        else close();
        return true;
      case "ArrowDown":
        e.preventDefault();
        if (expanded && selectedProfile < expanded.profiles.length - 1) selectedProfile++;
        else if (selectedIndex < visibleBrowsers.length - 1) selectRow(selectedIndex + 1);
        return true;
      case "ArrowUp":
        e.preventDefault();
        if (expanded && selectedProfile > 0) selectedProfile--;
        else if (selectedIndex > 0) selectRow(selectedIndex - 1);
        return true;
      case "ArrowRight":
      case "Tab":
        if (current?.profiles.length) {
          e.preventDefault();
          toggleProfiles(current, true);
        }
        return true;
      case "ArrowLeft":
        if (expanded) {
          e.preventDefault();
          toggleProfiles(expanded, false);
        }
        return true;
      case "Enter": {
        e.preventDefault();
        if (!current) return true;
        const profileId = expanded && selectedProfile >= 0 ? expanded.profiles[selectedProfile].id : null;
        open(current, profileId, e.shiftKey);
        return true;
      }
      case "Backspace":
        if (filter) {
          e.preventDefault();
          filter = filter.slice(0, -1);
          selectedIndex = 0;
          resize();
        }
        return true;
    }
    return false;
  }

  function onKey(e: KeyboardEvent) {
    if (!pending) return;
    privateHeld = e.shiftKey;
    const mod = e.ctrlKey || e.metaKey;

    if (mod && e.key.toLowerCase() === "c" && !window.getSelection()?.toString()) {
      e.preventDefault();
      copyUrl();
      return;
    }
    if (mod && e.key === ",") {
      e.preventDefault();
      api.openSettings();
      return;
    }
    if (mode === "rule") {
      onRuleKey(e);
      return;
    }

    const current = visibleBrowsers[selectedIndex];
    const expanded = current && expandedId === current.id ? current : null;
    if (onListNavKey(e, current, expanded)) return;
    if (mod || e.altKey) return;

    const digit = /^Digit([1-9])$/.exec(e.code);
    if (digit) {
      e.preventDefault();
      const b = visibleBrowsers[Number(digit[1]) - 1];
      if (b) open(b, null, e.shiftKey);
      return;
    }

    const key = e.key.toLowerCase();
    if (expanded && PROFILE_KEYS.includes(key)) {
      const profile = expanded.profiles[PROFILE_KEYS.indexOf(key)];
      if (profile) {
        e.preventDefault();
        open(expanded, profile.id, e.shiftKey);
      }
      return;
    }

    if (filterEnabled && e.key.length === 1 && /\S/.test(e.key)) {
      e.preventDefault();
      filter += e.key;
      selectedIndex = 0;
      expandedId = null;
      resize();
    }
  }

  onMount(() => {
    const unlisteners = [
      listen<Pending>("picker:show", (ev) => show(ev.payload)),
      listen("settings:changed", async () => (snapshot = await api.snapshot())),
    ];
    api.pending().then((next) => next && show(next));
    const onKeyUp = (e: KeyboardEvent) => (privateHeld = e.shiftKey);
    const onBlur = () => (privateHeld = false);
    addEventListener("keyup", onKeyUp);
    addEventListener("blur", onBlur);
    return () => {
      unlisteners.forEach((u) => u.then((stop) => stop()));
      removeEventListener("keyup", onKeyUp);
      removeEventListener("blur", onBlur);
    };
  });

  /** Splits a name around the first filter match so the match can be highlighted. */
  function highlight(name: string) {
    const at = filter ? name.toLowerCase().indexOf(filter.toLowerCase()) : -1;
    if (at < 0) return [{ text: name, match: false }];
    const end = at + filter.length;
    return [
      { text: name.slice(0, at), match: false },
      { text: name.slice(at, end), match: true },
      { text: name.slice(end), match: false },
    ];
  }
</script>

<svelte:window onkeydown={onKey} />

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<section class="picker" bind:this={card} tabindex="0" aria-label="Choose a browser">
  {#if pending && urlParts}
    <header class="url" data-tauri-drag-region>
      {#if urlParts.secure}
        <svg class="lock" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-label="Secure"><rect x="5" y="11" width="14" height="9" rx="2" /><path d="M8 11V8a4 4 0 0 1 8 0v3" /></svg>
      {:else}
        <span class="scheme" title="Not HTTPS">{urlParts.scheme.replace(":", "")}</span>
      {/if}
      <button class="u" title={`${pending.url}\n(${modKey(os)}+C to copy)`} onclick={copyUrl} data-tauri-drag-region>
        <b>{urlParts.host}</b><span class="p1">{urlParts.mid}</span><span class="p2">{urlParts.tail}</span>
      </button>
      {#if copied}<span class="chip info">copied</span>
      {:else if mode === "rule"}<span class="chip rule"><svg width="11" height="11" viewBox="0 0 24 24" fill="currentColor"><path d="M13 2 4 14h7l-1 8 9-12h-7z" /></svg>RULE</span>
      {:else if privateHeld}<span class="chip priv"><svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2"><path d="M3 8c3-1 6-1 9 1 3-2 6-2 9-1-1 6-3 8-6 8-2 0-2-2-3-2s-1 2-3 2c-3 0-5-2-6-8z" /></svg>PRIVATE</span>
      {:else if pending.removed}<span class="chip info" title="Tracking parameters removed">−{pending.removed} tracker{pending.removed > 1 ? "s" : ""}</span>{/if}
    </header>

    {#if pending.lookalike}
      <div class="warn" role="alert">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 3 2 21h20zM12 10v5M12 18v.5" /></svg>
        <span>Lookalike domain: <code>{urlParts.host}</code> resembles <code>{pending.lookalike}</code>. Check before opening.</span>
      </div>
    {/if}

    {#if mode === "rule" && ruleBrowser}
      <div class="rule">
        <div class="hd">
          <Glyph id={ruleBrowser.id} family={ruleBrowser.family} name={ruleBrowser.name} size={28} />
          <div>
            <div class="t">Opening in {ruleTitle}</div>
            <div class="s">rule <code>{matchedRule?.pattern}</code>{countdownLabel}</div>
          </div>
        </div>
        <div class="bar" aria-hidden="true"><i style="transform:scaleX({countdownProgress})"></i></div>
        <div class="act">
          <button class="btn pri" onclick={openRule}>Open now <span class="kbd">Enter</span></button>
          <button class="btn" onclick={showList}>Choose another <span class="kbd">Space</span></button>
          <button class="btn ghost" onclick={close}>Cancel</button>
        </div>
      </div>
    {:else}
      {#if filterEnabled}
        <div class="filter" class:active={!!filter}>
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2"><circle cx="11" cy="11" r="7" /><path d="m20 20-4-4" /></svg>
          <span class="q">{filter || "Type to filter"}</span>{#if filter}<i class="cur"></i>{/if}
          <span class="n">{visibleBrowsers.length} of {enabledBrowsers.length}</span>
        </div>
      {/if}
      <ul class="list" role="listbox" aria-label="Browsers">
        {#each visibleBrowsers as b, i (b.id)}
          {@const isExpanded = expandedId === b.id}
          <!-- svelte-ignore a11y_click_events_have_key_events (keyboard handled on window) -->
          <li
            class="row"
            class:on={i === selectedIndex && (!isExpanded || selectedProfile < 0)}
            role="option"
            aria-selected={i === selectedIndex}
            onmouseenter={() => {
              if (!isExpanded) selectedIndex = i;
            }}
            onclick={(e) => open(b, null, e.shiftKey)}
          >
            <Glyph id={b.id} family={b.family} name={b.name} />
            <span class="nm">
              {#each highlight(b.name) as part}{#if part.match}<mark>{part.text}</mark>{:else}{part.text}{/if}{/each}
              {#if privateHeld}<small> {privateModeName(b)}</small>{:else if b.profiles.length}<small> {b.profiles.length} profiles</small>{/if}
            </span>
            {#if b.profiles.length}
              <button
                class="caret"
                class:open={isExpanded}
                aria-label="Profiles"
                onclick={(e) => {
                  e.stopPropagation();
                  selectedIndex = i;
                  toggleProfiles(b);
                }}
              >
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4"><path d="m9 6 6 6-6 6" /></svg>
              </button>
            {/if}
            <span class="kbd">{i < HOTKEY_COUNT ? i + 1 : "·"}</span>
          </li>
          {#if isExpanded}
            {#each b.profiles as profile, j (profile.id)}
              <!-- svelte-ignore a11y_click_events_have_key_events -->
              <li
                class="row sub"
                class:on={selectedProfile === j}
                role="option"
                aria-selected={selectedProfile === j}
                onmouseenter={() => (selectedProfile = j)}
                onclick={(e) => open(b, profile.id, e.shiftKey)}
              >
                <span class="dot" style="background:hsl({(j * 97 + 20) % 360} 60% 55%)"></span>
                <span class="nm">{profile.name}</span>
                <span class="kbd">{PROFILE_KEYS[j]?.toUpperCase() ?? "·"}</span>
              </li>
            {/each}
          {/if}
        {:else}
          <li class="empty">{#if enabledBrowsers.length}No browser matches{:else}No browsers found — open Settings to rescan{/if}</li>
        {/each}
      </ul>
    {/if}

    {#if error}<div class="err" role="alert">{error}</div>{/if}

    <footer class="foot">
      {#if mode === "rule"}
        <span><span class="kbd">Esc</span> show all browsers</span>
      {:else}
        <span><span class="kbd">1</span>–<span class="kbd">9</span> open</span>
        <span><span class="kbd">{os === "macos" ? "⇧" : "Shift"}</span> {privateHeld ? "held" : "private"}</span>
        {#if expandedId}<span><span class="kbd">Q</span><span class="kbd">W</span><span class="kbd">E</span> profile</span>
        {:else if anyProfiles}<span><span class="kbd">→</span> profiles</span>{/if}
        <span class="r"><span class="kbd">Esc</span></span>
      {/if}
    </footer>
  {/if}
</section>

<style>
  :global(html), :global(body) { background: transparent; overflow: hidden; }
  .picker {
    width: 440px; background: var(--surf); color: var(--tx); border: 1px solid var(--line2);
    border-radius: var(--rad); overflow: hidden; outline: none;
  }
  :global([data-os="linux"]) .picker { border-color: rgba(127, 127, 127, 0.35); }

  /* URL header */
  .url { display: flex; align-items: center; gap: 8px; padding: 10px 14px; border-bottom: 1px solid var(--line); font: 13px/20px var(--mono); min-width: 0; }
  .lock { flex: none; color: var(--tx3); }
  .scheme { flex: none; font: 600 11px/16px var(--mono); color: var(--sigtx); text-transform: uppercase; }
  .u { display: flex; min-width: 0; flex: 1; color: var(--tx3); white-space: nowrap; background: none; border: 0; text-align: left; font: inherit; cursor: copy; }
  .u b { color: var(--tx); font-weight: 600; flex: none; max-width: 70%; overflow: hidden; text-overflow: ellipsis; }
  .u .p1 { overflow: hidden; text-overflow: ellipsis; min-width: 0; flex: 0 1 auto; }
  .u .p2 { flex: none; }
  .warn { display: flex; gap: 8px; align-items: flex-start; margin: 8px 8px 0; padding: 8px 10px; border-radius: 6px; background: color-mix(in srgb, var(--warn) 14%, transparent); border: 1px solid color-mix(in srgb, var(--warn) 55%, transparent); font-size: 12px; line-height: 18px; }
  .warn svg { flex: none; margin-top: 2px; color: var(--warn); }
  .warn code { font: 600 12px var(--mono); }

  /* Browser list */
  .list { padding: 4px; list-style: none; max-height: 376px; overflow-y: auto; scrollbar-width: thin; }
  .row { position: relative; display: flex; align-items: center; gap: 12px; height: 40px; padding: 0 12px; border-radius: 6px; cursor: pointer; }
  .row.on { background: var(--sel); }
  .row.on::before { content: ""; position: absolute; left: 0; top: 8px; bottom: 8px; width: 3px; border-radius: 2px; background: var(--sig); }
  .row.on .kbd { color: var(--tx); border-color: var(--tx3); }
  .nm { font-weight: 500; flex: 1; min-width: 0; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .nm small { font-weight: 400; color: var(--tx3); font-size: 13px; }
  .nm mark { background: none; color: var(--sigtx); font-weight: 700; text-decoration: underline; }
  .row.sub { height: 32px; padding-left: 48px; }
  .row.sub .nm { font-weight: 400; font-size: 13px; }
  .dot { width: 8px; height: 8px; border-radius: 50%; flex: none; }
  .caret { background: none; border: 0; color: var(--tx3); display: grid; place-items: center; width: 20px; height: 20px; border-radius: 4px; cursor: pointer; }
  .caret:hover { background: var(--surf2); color: var(--tx); }
  .caret svg { transition: transform 0.12s; }
  .caret.open svg { transform: rotate(90deg); }
  .empty { padding: 14px 12px; color: var(--tx3); font-size: 13px; }
  .filter { display: flex; align-items: center; gap: 8px; margin: 6px 6px 2px; padding: 0 10px; height: 32px; border: 1px solid var(--line2); border-radius: 6px; background: var(--surf2); font: 13px var(--mono); color: var(--tx3); }
  .filter.active { border: 2px solid var(--ring); color: var(--tx); }
  .cur { width: 1px; height: 16px; background: var(--tx); animation: blink 1s steps(1) infinite; }
  @keyframes blink { 50% { opacity: 0; } }
  .filter .n { margin-left: auto; font-size: 11px; color: var(--tx3); }

  /* Rule countdown */
  .rule { padding: 16px; }
  .hd { display: flex; align-items: center; gap: 12px; }
  .t { font-weight: 600; font-size: 15px; }
  .s { color: var(--tx2); font-size: 13px; }
  .s code { font: 12px var(--mono); color: var(--tx); }
  .bar { height: 3px; background: var(--line); border-radius: 2px; margin: 14px 0 12px; overflow: hidden; }
  .bar i { display: block; height: 100%; background: var(--sig); transform-origin: left; }
  .act { display: flex; gap: 8px; flex-wrap: wrap; }

  .err { margin: 0 8px 8px; padding: 8px 10px; border-radius: 6px; font-size: 12px; color: var(--sigtx); background: color-mix(in srgb, var(--sig) 10%, transparent); }
  .foot { display: flex; align-items: center; gap: 14px; padding: 8px 14px; border-top: 1px solid var(--line); background: var(--surf2); font: 11px/16px var(--mono); color: var(--tx3); }
  .foot span { display: inline-flex; align-items: center; gap: 4px; }
  .foot .r { margin-left: auto; }
  .foot .kbd { height: 18px; min-width: 18px; }
</style>
