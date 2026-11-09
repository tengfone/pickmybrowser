<script lang="ts">
  import { onMount, tick } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { api, applyTheme, splitUrl, PROFILE_KEYS, modKey, type Browser, type Pending, type Snapshot } from "./api";
  import Glyph from "./Glyph.svelte";

  let snap = $state<Snapshot | null>(null);
  let p = $state<Pending | null>(null);
  let sel = $state(0);
  let expanded = $state<string | null>(null);
  let profSel = $state(-1);
  let priv = $state(false);
  let filter = $state("");
  let mode = $state<"list" | "rule">("list");
  let progress = $state(0);
  let error = $state("");
  let copied = $state(false);
  let card: HTMLElement;
  let raf = 0;

  const all = $derived((snap?.browsers ?? []).filter((b) => b.enabled));
  const useFilter = $derived(all.length > 9);
  const list = $derived(
    filter ? all.filter((b) => b.name.toLowerCase().includes(filter.toLowerCase())) : all,
  );
  const parts = $derived(p ? splitUrl(p.url) : null);
  const ruleBrowser = $derived(p?.rule ? all.find((b) => b.id === p!.rule!.browserId) : undefined);
  const ruleProfile = $derived(
    ruleBrowser && p?.rule?.profileId ? ruleBrowser.profiles.find((x) => x.id === p!.rule!.profileId) : undefined,
  );
  const os = $derived(snap?.os ?? "windows");
  const privWord = (b: Browser) =>
    b.engine === "chromium" ? (b.family === "edge" ? "InPrivate" : b.family === "chrome" ? "Incognito" : "Private") : "Private";

  async function show(next: Pending) {
    cancelAnimationFrame(raf);
    snap = await api.snapshot();
    applyTheme(snap.settings.theme);
    p = next;
    sel = 0; expanded = null; profSel = -1; filter = ""; error = ""; copied = false; priv = false;
    mode = next.rule && ruleBrowser ? "rule" : "list";
    await tick();
    await api.present(card.offsetWidth, card.offsetHeight);
    card.focus();
    if (mode === "rule") startCountdown();
  }

  function startCountdown() {
    const ms = p?.countdownMs ?? 0;
    if (p?.lookalike || ms <= 0) { progress = 0; return; }
    const t0 = performance.now();
    const step = (t: number) => {
      progress = Math.min(1, (t - t0) / ms);
      if (mode !== "rule") return;
      if (progress >= 1) openRule();
      else raf = requestAnimationFrame(step);
    };
    raf = requestAnimationFrame(step);
  }

  async function resize() {
    await tick();
    if (card) await api.present(card.offsetWidth, card.offsetHeight);
  }

  async function go(b: Browser, profileId: string | null = null, isPriv = priv) {
    if (!p) return;
    cancelAnimationFrame(raf);
    try {
      await api.open(p.url, b.id, profileId, isPriv);
      p = null;
    } catch (e) {
      error = String(e);
      resize();
    }
  }
  const openRule = () => ruleBrowser && go(ruleBrowser, p?.rule?.profileId ?? null, false);

  function toList() {
    cancelAnimationFrame(raf);
    mode = "list";
    const i = list.findIndex((b) => b.id === ruleBrowser?.id);
    sel = Math.max(0, i);
    resize();
  }

  function close() {
    cancelAnimationFrame(raf);
    p = null;
    api.dismiss();
  }

  function toggleProfiles(b: Browser, open?: boolean) {
    const want = open ?? expanded !== b.id;
    expanded = want && b.profiles.length ? b.id : null;
    profSel = expanded ? 0 : -1;
    resize();
  }

  async function copy() {
    if (!p) return;
    await navigator.clipboard.writeText(p.url);
    copied = true;
    setTimeout(() => (copied = false), 1200);
  }

  function onKey(e: KeyboardEvent) {
    if (!p) return;
    priv = e.shiftKey;
    const mod = e.ctrlKey || e.metaKey;
    if (mod && e.key.toLowerCase() === "c" && !window.getSelection()?.toString()) { e.preventDefault(); copy(); return; }
    if (mod && e.key === ",") { e.preventDefault(); api.openSettings(); return; }
    if (mode === "rule") {
      if (e.key === "Enter") { e.preventDefault(); openRule(); }
      else if (e.key === " ") { e.preventDefault(); toList(); }
      else if (e.key === "Escape") { e.preventDefault(); toList(); }
      return;
    }
    const cur = list[sel];
    const exp = cur && expanded === cur.id ? cur : null;
    switch (e.key) {
      case "Escape":
        e.preventDefault();
        if (exp) toggleProfiles(cur, false);
        else if (filter) filter = "";
        else close();
        return;
      case "ArrowDown":
        e.preventDefault();
        if (exp && profSel < exp.profiles.length - 1) profSel++;
        else if (sel < list.length - 1) { sel++; expanded = null; profSel = -1; resize(); }
        return;
      case "ArrowUp":
        e.preventDefault();
        if (exp && profSel > 0) profSel--;
        else if (sel > 0) { sel--; expanded = null; profSel = -1; resize(); }
        return;
      case "ArrowRight":
      case "Tab":
        if (cur?.profiles.length) { e.preventDefault(); toggleProfiles(cur, true); }
        return;
      case "ArrowLeft":
        if (exp) { e.preventDefault(); toggleProfiles(cur, false); }
        return;
      case "Enter":
        e.preventDefault();
        if (cur) go(cur, exp && profSel >= 0 ? exp.profiles[profSel].id : null, e.shiftKey);
        return;
      case "Backspace":
        if (filter) { e.preventDefault(); filter = filter.slice(0, -1); sel = 0; resize(); }
        return;
    }
    if (mod || e.altKey) return;
    const d = /^Digit([1-9])$/.exec(e.code);
    if (d) {
      e.preventDefault();
      const b = list[+d[1] - 1];
      if (b) go(b, null, e.shiftKey);
      return;
    }
    const k = e.key.toLowerCase();
    if (exp && PROFILE_KEYS.includes(k)) {
      const pr = exp.profiles[PROFILE_KEYS.indexOf(k)];
      if (pr) { e.preventDefault(); go(exp, pr.id, e.shiftKey); }
      return;
    }
    if (useFilter && e.key.length === 1 && /\S/.test(e.key)) {
      e.preventDefault();
      filter += e.key;
      sel = 0; expanded = null;
      resize();
    }
  }

  onMount(() => {
    const un = [
      listen<Pending>("picker:show", (ev) => show(ev.payload)),
      listen("settings:changed", async () => (snap = await api.snapshot())),
    ];
    api.pending().then((x) => x && show(x));
    const up = (e: KeyboardEvent) => (priv = e.shiftKey);
    const blur = () => (priv = false);
    addEventListener("keyup", up);
    addEventListener("blur", blur);
    return () => {
      un.forEach((u) => u.then((f) => f()));
      removeEventListener("keyup", up);
      removeEventListener("blur", blur);
    };
  });

  function hl(name: string) {
    if (!filter) return [{ t: name, m: false }];
    const i = name.toLowerCase().indexOf(filter.toLowerCase());
    if (i < 0) return [{ t: name, m: false }];
    return [
      { t: name.slice(0, i), m: false },
      { t: name.slice(i, i + filter.length), m: true },
      { t: name.slice(i + filter.length), m: false },
    ];
  }
</script>

<svelte:window onkeydown={onKey} />

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<section class="picker" bind:this={card} tabindex="0" aria-label="Choose a browser">
  {#if p && parts}
    <header class="url" data-tauri-drag-region>
      {#if parts.secure}
        <svg class="lock" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-label="Secure"><rect x="5" y="11" width="14" height="9" rx="2" /><path d="M8 11V8a4 4 0 0 1 8 0v3" /></svg>
      {:else}
        <span class="scheme" title="Not HTTPS">{parts.scheme.replace(":", "")}</span>
      {/if}
      <button class="u" title={p.url + "\n(" + modKey(os) + "+C to copy)"} onclick={copy} data-tauri-drag-region>
        <b>{parts.host}</b><span class="p1">{parts.mid}</span><span class="p2">{parts.tail}</span>
      </button>
      {#if copied}<span class="chip info">copied</span>
      {:else if mode === "rule"}<span class="chip rule"><svg width="11" height="11" viewBox="0 0 24 24" fill="currentColor"><path d="M13 2 4 14h7l-1 8 9-12h-7z" /></svg>RULE</span>
      {:else if priv}<span class="chip priv"><svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2"><path d="M3 8c3-1 6-1 9 1 3-2 6-2 9-1-1 6-3 8-6 8-2 0-2-2-3-2s-1 2-3 2c-3 0-5-2-6-8z" /></svg>PRIVATE</span>
      {:else if p.removed}<span class="chip info" title="Tracking parameters removed">−{p.removed} tracker{p.removed > 1 ? "s" : ""}</span>{/if}
    </header>

    {#if p.lookalike}
      <div class="warn" role="alert">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 3 2 21h20zM12 10v5M12 18v.5" /></svg>
        <span>Lookalike domain: <code>{parts.host}</code> resembles <code>{p.lookalike}</code>. Check before opening.</span>
      </div>
    {/if}

    {#if mode === "rule" && ruleBrowser}
      <div class="rule">
        <div class="hd">
          <Glyph family={ruleBrowser.family} name={ruleBrowser.name} size={28} />
          <div>
            <div class="t">Opening in {ruleBrowser.name}{ruleProfile ? ` · ${ruleProfile.name}` : ""}</div>
            <div class="s">rule <code>{p.rule?.pattern}</code>{p.countdownMs && !p.lookalike ? ` · ${(p.countdownMs / 1000).toFixed(1)} s` : ""}</div>
          </div>
        </div>
        <div class="bar" aria-hidden="true"><i style="transform:scaleX({progress})"></i></div>
        <div class="act">
          <button class="btn pri" onclick={openRule}>Open now <span class="kbd">Enter</span></button>
          <button class="btn" onclick={toList}>Choose another <span class="kbd">Space</span></button>
          <button class="btn ghost" onclick={close}>Cancel</button>
        </div>
      </div>
    {:else}
      {#if useFilter}
        <div class="filter" class:active={!!filter}>
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2"><circle cx="11" cy="11" r="7" /><path d="m20 20-4-4" /></svg>
          <span class="q">{filter || "Type to filter"}</span>{#if filter}<i class="cur"></i>{/if}
          <span class="n">{list.length} of {all.length}</span>
        </div>
      {/if}
      <ul class="list" role="listbox" aria-label="Browsers">
        {#each list as b, i (b.id)}
          <!-- svelte-ignore a11y_click_events_have_key_events (keyboard handled on window) -->
          <li
            class="row"
            class:on={i === sel && (expanded !== b.id || profSel < 0)}
            role="option"
            aria-selected={i === sel}
            onmouseenter={() => { if (expanded !== b.id) { sel = i; } }}
            onclick={(e) => go(b, null, e.shiftKey)}
          >
            <Glyph family={b.family} name={b.name} />
            <span class="nm">
              {#each hl(b.name) as s}{#if s.m}<mark>{s.t}</mark>{:else}{s.t}{/if}{/each}
              {#if priv}<small> {privWord(b)}</small>{:else if b.profiles.length}<small> {b.profiles.length} profiles</small>{/if}
            </span>
            {#if b.profiles.length}
              <button class="caret" class:open={expanded === b.id} aria-label="Profiles" onclick={(e) => { e.stopPropagation(); sel = i; toggleProfiles(b); }}>
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4"><path d="m9 6 6 6-6 6" /></svg>
              </button>
            {/if}
            <span class="kbd">{i < 9 ? i + 1 : "·"}</span>
          </li>
          {#if expanded === b.id}
            {#each b.profiles as pr, j (pr.id)}
              <!-- svelte-ignore a11y_click_events_have_key_events -->
              <li class="row sub" class:on={profSel === j} role="option" aria-selected={profSel === j}
                onmouseenter={() => (profSel = j)} onclick={(e) => go(b, pr.id, e.shiftKey)}>
                <span class="dot" style="background:hsl({(j * 97 + 20) % 360} 60% 55%)"></span>
                <span class="nm">{pr.name}</span>
                <span class="kbd">{PROFILE_KEYS[j]?.toUpperCase() ?? "·"}</span>
              </li>
            {/each}
          {/if}
        {:else}
          <li class="empty">{all.length ? "No browser matches" : "No browsers found — open Settings to rescan"}</li>
        {/each}
      </ul>
    {/if}

    {#if error}<div class="err" role="alert">{error}</div>{/if}

    <footer class="foot">
      {#if mode === "rule"}
        <span><span class="kbd">Esc</span> show all browsers</span>
      {:else}
        <span><span class="kbd">1</span>–<span class="kbd">9</span> open</span>
        <span><span class="kbd">{os === "macos" ? "⇧" : "Shift"}</span> {priv ? "held" : "private"}</span>
        {#if expanded}<span><span class="kbd">Q</span><span class="kbd">W</span><span class="kbd">E</span> profile</span>
        {:else if list.some((b) => b.profiles.length)}<span><span class="kbd">→</span> profiles</span>{/if}
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
