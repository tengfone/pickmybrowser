<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { api, applyTheme, type Browser, type Rule, type Snapshot, type Settings } from "./api";
  import Glyph from "./Glyph.svelte";

  let snap = $state<Snapshot | null>(null);
  let tab = $state<"browsers" | "rules" | "recents" | "general" | "about">("browsers");
  let step = $state(1);
  let busy = $state(false);
  let dragFrom = $state<number | null>(null);
  let dragOver = $state<number | null>(null);
  let confirmReset = $state(false);
  let toast = $state("");

  const tabs = [
    ["browsers", "Browsers"], ["rules", "Rules"], ["recents", "Recent links"], ["general", "General"], ["about", "About"],
  ] as const;

  async function load() {
    snap = await api.snapshot();
    applyTheme(snap.settings.theme);
  }
  onMount(() => {
    load();
    const un = listen("tauri://focus", () => load());
    const mq = matchMedia("(prefers-color-scheme: dark)");
    const onMq = () => snap && applyTheme(snap.settings.theme);
    mq.addEventListener("change", onMq);
    return () => { un.then((f) => f()); mq.removeEventListener("change", onMq); };
  });

  function flash(msg: string) { toast = msg; setTimeout(() => (toast = ""), 1800); }

  /* browsers */
  const saveBrowsers = () => snap && api.saveBrowsers($state.snapshot(snap.browsers));
  async function rescan() {
    if (!snap) return;
    busy = true;
    snap.browsers = await api.rescan();
    busy = false;
    flash(`Found ${snap.browsers.length} browser${snap.browsers.length === 1 ? "" : "s"}`);
  }
  function move(from: number, to: number) {
    if (!snap || from === to || to < 0 || to >= snap.browsers.length) return;
    const list = [...snap.browsers];
    const [b] = list.splice(from, 1);
    list.splice(to, 0, b);
    snap.browsers = list;
    saveBrowsers();
  }
  function toggleBrowser(b: Browser) { b.enabled = !b.enabled; saveBrowsers(); }
  const enabledIndex = (b: Browser) => snap!.browsers.filter((x) => x.enabled).indexOf(b);

  /* rules */
  const saveRules = () => snap && api.saveRules($state.snapshot(snap.rules));
  function addRule() {
    if (!snap) return;
    snap.rules = [...snap.rules, { pattern: "", browserId: snap.browsers[0]?.id ?? "", profileId: null, enabled: true }];
  }
  function removeRule(i: number) { if (!snap) return; snap.rules = snap.rules.filter((_, j) => j !== i); saveRules(); }
  function ruleTarget(r: Rule) { return r.profileId ? `${r.browserId}::${r.profileId}` : r.browserId; }
  function setRuleTarget(r: Rule, v: string) {
    const [b, p] = v.split("::");
    r.browserId = b; r.profileId = p ?? null; saveRules();
  }

  /* settings */
  function setS<K extends keyof Settings>(k: K, v: Settings[K]) {
    if (!snap) return;
    snap.settings[k] = v;
    if (k === "theme") applyTheme(v as Settings["theme"]);
    api.saveSettings($state.snapshot(snap.settings));
  }
  async function makeDefault() {
    if (!snap) return;
    try {
      snap.isDefault = await api.setDefault();
      if (!snap.isDefault) {
        flash(snap.os === "windows" ? "Pick PickMyBrowser in Default apps" : "Waiting for confirmation…");
        const t = setInterval(async () => {
          if (!snap) return clearInterval(t);
          snap.isDefault = await api.isDefault();
          if (snap.isDefault) clearInterval(t);
        }, 1500);
        setTimeout(() => clearInterval(t), 60000);
      }
    } catch (e) { flash(String(e)); }
  }
  async function toggleLogin() {
    if (!snap) return;
    const on = !snap.launchAtLogin;
    try { await api.setLaunchAtLogin(on); snap.launchAtLogin = on; } catch (e) { flash(String(e)); }
  }
  async function reset() {
    if (!snap) return;
    await api.resetAll();
    confirmReset = false;
    await load();
    step = 1;
    flash("Everything reset");
  }
  function finishOnboarding() { setS("onboarded", true); }

  const fmtAgo = (s: number) => {
    const d = Date.now() / 1000 - s;
    if (d < 60) return "just now";
    if (d < 3600) return `${Math.floor(d / 60)}m ago`;
    if (d < 86400) return `${Math.floor(d / 3600)}h ago`;
    return `${Math.floor(d / 86400)}d ago`;
  };
  const bName = (id?: string | null) => snap?.browsers.find((b) => b.id === id);
</script>

{#snippet toggle(on: boolean, label: string, fn: () => void)}
  <button class="tog" role="switch" aria-checked={on} aria-label={label} onclick={fn}></button>
{/snippet}

{#if snap}
  {#if !snap.settings.onboarded}
    <!-- ---------------------------------------------------------------- onboarding -->
    <div class="onb">
      <div class="in">
        <div class="steps"><i class="d"></i><i class:d={step === 2}></i></div>
        {#if step === 1}
          <h1>We found {snap.browsers.length} browser{snap.browsers.length === 1 ? "" : "s"}</h1>
          <p>Uncheck any you don't want in the picker. You can reorder them later.</p>
          <div class="card">
            {#each snap.browsers as b (b.id)}
              <div class="srow" class:off={!b.enabled}>
                <Glyph family={b.family} name={b.name} />
                <span class="nm">{b.name}{#if b.profiles.length}<small>{b.profiles.length} profiles</small>{/if}</span>
                <span class="ok">{b.enabled ? "✓ found" : "hidden"}</span>
                {@render toggle(b.enabled, `Show ${b.name}`, () => toggleBrowser(b))}
              </div>
            {:else}
              <div class="srow"><span class="nm muted">No browsers detected yet.</span><button class="btn" onclick={rescan}>Rescan</button></div>
            {/each}
          </div>
        {:else}
          <h1>Make it your default</h1>
          <p>
            Links from any app will open the picker.
            {snap.os === "windows" ? "Windows will open Default apps — choose PickMyBrowser for HTTP and HTTPS." :
             snap.os === "macos" ? "macOS will ask you to confirm." : "We'll register with xdg-settings."}
          </p>
          <div class="card">
            <div class="srow">
              <span class="nm">Default browser<small>http · https</small></span>
              {#if snap.isDefault}<span class="status"><i></i>PickMyBrowser is default</span>
              {:else}<button class="btn pri" onclick={makeDefault}>Set as default</button>{/if}
            </div>
            <div class="srow">
              <span class="nm">Launch at login<small>keeps the picker instant</small></span>
              {@render toggle(snap.launchAtLogin, "Launch at login", toggleLogin)}
            </div>
            <div class="srow">
              <span class="nm">Try it<small>open a sample link</small></span>
              <button class="btn" onclick={() => api.pick("https://github.com/tengfone/PickMyBrowser?utm_source=onboarding")}>Test picker</button>
            </div>
          </div>
        {/if}
      </div>
      <div class="ft">
        <small>Step {step} of 2</small>
        {#if step === 1}
          <button class="btn pri" onclick={() => (step = 2)} disabled={!snap.browsers.length}>Continue</button>
        {:else}
          <span class="row-gap"><button class="btn ghost" onclick={() => (step = 1)}>Back</button><button class="btn pri" onclick={finishOnboarding}>Done</button></span>
        {/if}
      </div>
    </div>
  {:else}
    <!-- ---------------------------------------------------------------- settings -->
    <div class="shell">
      <nav class="side" aria-label="Settings sections">
        {#each tabs as [id, label]}
          <button class:on={tab === id} aria-current={tab === id ? "page" : undefined} onclick={() => (tab = id)}>{label}</button>
        {/each}
        <div class="ver">
          v{snap.version}{#if snap.isDefault} · default ✓{:else}<br /><button class="linkish" onclick={() => (tab = "general")}><i class="dotw"></i>not default</button>{/if}
        </div>
      </nav>

      <main class="pane">
        {#if tab === "browsers"}
          <div class="head">
            <div><h1>Browsers</h1><p class="sub">Drag to reorder. Order sets the 1–9 hotkeys.</p></div>
            <button class="btn" onclick={rescan} disabled={busy}>
              <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" class:spin={busy}><path d="M20 12a8 8 0 1 1-2.3-5.7M20 4v5h-5" /></svg>
              {busy ? "Scanning…" : "Rescan"}
            </button>
          </div>
          <div class="sec">Detected · {snap.browsers.length}</div>
          <div class="card" role="list">
            {#each snap.browsers as b, i (b.id)}
              <div
                class="srow" class:off={!b.enabled} class:over={dragOver === i && dragFrom !== i} role="listitem"
                draggable="true"
                ondragstart={(e) => { dragFrom = i; e.dataTransfer?.setData("text/plain", String(i)); }}
                ondragover={(e) => { e.preventDefault(); dragOver = i; }}
                ondragend={() => { dragFrom = null; dragOver = null; }}
                ondrop={(e) => { e.preventDefault(); if (dragFrom !== null) move(dragFrom, i); dragFrom = null; dragOver = null; }}
              >
                <span class="grip" aria-hidden="true">⠿</span>
                <Glyph family={b.family} name={b.name} />
                <span class="nm">{b.name}<small title={b.exec}>{b.exec}{b.profiles.length ? ` · ${b.profiles.length} profiles` : ""}</small></span>
                <span class="mv">
                  <button class="icon" aria-label="Move up" disabled={i === 0} onclick={() => move(i, i - 1)}>↑</button>
                  <button class="icon" aria-label="Move down" disabled={i === snap.browsers.length - 1} onclick={() => move(i, i + 1)}>↓</button>
                </span>
                <span class="kbd">{b.enabled && enabledIndex(b) < 9 ? enabledIndex(b) + 1 : "–"}</span>
                {@render toggle(b.enabled, `Show ${b.name} in picker`, () => toggleBrowser(b))}
              </div>
            {:else}
              <div class="srow"><span class="nm muted">No browsers found. Click Rescan.</span></div>
            {/each}
          </div>
          <p class="hint">Profiles are detected automatically for Chromium and Firefox browsers. Press <span class="kbd">→</span> in the picker to choose one.</p>

        {:else if tab === "rules"}
          <div class="head">
            <div><h1>Rules</h1><p class="sub">First match wins. Matching links skip the picker after a short countdown.</p></div>
            <button class="btn pri" onclick={addRule}>+ Add rule</button>
          </div>
          <div class="sec">{snap.rules.length} rule{snap.rules.length === 1 ? "" : "s"}</div>
          <div class="card">
            {#each snap.rules as r, i}
              <div class="srow">
                <input class="pat" placeholder="*.example.com or github.com/acme/*" bind:value={r.pattern} onchange={saveRules} spellcheck="false" aria-label="URL pattern" />
                <span class="arrow">→</span>
                <select class="sel" value={ruleTarget(r)} onchange={(e) => setRuleTarget(r, e.currentTarget.value)} aria-label="Open with">
                  {#each snap.browsers as b}
                    <option value={b.id}>{b.name}</option>
                    {#each b.profiles as p}<option value={`${b.id}::${p.id}`}>{b.name} · {p.name}</option>{/each}
                  {/each}
                </select>
                {@render toggle(r.enabled, "Rule enabled", () => { r.enabled = !r.enabled; saveRules(); })}
                <button class="icon" aria-label="Delete rule" onclick={() => removeRule(i)}>✕</button>
              </div>
            {:else}
              <div class="srow empty"><span class="nm muted">No rules yet. Example: <code>meet.google.com</code> → Chrome · Work</span></div>
            {/each}
          </div>
          <div class="sec">Countdown</div>
          <div class="seg" role="group" aria-label="Countdown">
            {#each [[0, "Off"], [800, "0.8 s"], [1200, "1.2 s"], [2000, "2 s"]] as [ms, l]}
              <button aria-pressed={snap.settings.ruleCountdownMs === ms} onclick={() => setS("ruleCountdownMs", ms as number)}>{l}</button>
            {/each}
          </div>
          <p class="hint">Off opens immediately. Lookalike-domain warnings always stop and show the picker.</p>

        {:else if tab === "recents"}
          <div class="head">
            <div><h1>Recent links</h1><p class="sub">Stored only on this device.</p></div>
            <button class="btn" onclick={async () => { await api.clearRecents(); snap!.recents = []; }} disabled={!snap.recents.length}>Clear</button>
          </div>
          <div class="card">
            {#each snap.recents as r}
              {@const b = bName(r.browserId)}
              <div class="srow">
                {#if b}<Glyph family={b.family} name={b.name} size={20} />{:else}<span class="ph"></span>{/if}
                <span class="nm url" title={r.url}>{r.url.replace(/^https?:\/\//, "")}<small>{fmtAgo(r.at)}{b ? ` · ${b.name}` : ""}</small></span>
                <button class="icon" aria-label="Copy" onclick={() => { navigator.clipboard.writeText(r.url); flash("Copied"); }}>⧉</button>
                <button class="btn" onclick={() => api.pick(r.url)}>Open…</button>
              </div>
            {:else}
              <div class="srow"><span class="nm muted">{snap.settings.storeRecents ? "Nothing yet." : "Recent links are turned off in General."}</span></div>
            {/each}
          </div>

        {:else if tab === "general"}
          <h1>General</h1>
          <div class="sec">System</div>
          <div class="card">
            <div class="srow">
              <span class="nm">Default browser<small>{snap.os === "linux" ? "xdg-settings · " : ""}http, https</small></span>
              {#if snap.isDefault}<span class="status"><i></i>PickMyBrowser is default</span>
              {:else}<span class="status warn"><i></i>Not default</span><button class="btn pri" onclick={makeDefault}>Set as default</button>{/if}
            </div>
            <div class="srow"><span class="nm">Launch at login</span>{@render toggle(snap.launchAtLogin, "Launch at login", toggleLogin)}</div>
            <div class="srow"><span class="nm">Show {snap.os === "macos" ? "menu bar" : "tray"} icon</span>{@render toggle(snap.settings.showTray, "Show tray icon", () => setS("showTray", !snap!.settings.showTray))}</div>
          </div>
          <div class="sec">Picker</div>
          <div class="card">
            <div class="srow"><span class="nm">Position</span>
              <div class="seg" role="group" aria-label="Position">
                <button aria-pressed={snap.settings.position === "cursor"} onclick={() => setS("position", "cursor")}>At cursor</button>
                <button aria-pressed={snap.settings.position === "center"} onclick={() => setS("position", "center")}>Center</button>
              </div>
            </div>
            <div class="srow"><span class="nm">Theme</span>
              <div class="seg" role="group" aria-label="Theme">
                {#each [["system", "System"], ["dark", "Dark"], ["light", "Light"]] as [v, l]}
                  <button aria-pressed={snap.settings.theme === v} onclick={() => setS("theme", v as Settings["theme"])}>{l}</button>
                {/each}
              </div>
            </div>
          </div>
          <div class="sec">Privacy & safety</div>
          <div class="card">
            <div class="srow"><span class="nm">Strip tracking parameters<small>utm_*, fbclid, gclid, …</small></span>{@render toggle(snap.settings.stripTracking, "Strip tracking", () => setS("stripTracking", !snap!.settings.stripTracking))}</div>
            <div class="srow"><span class="nm">Warn on lookalike domains<small>e.g. paypa1.com</small></span>{@render toggle(snap.settings.warnLookalike, "Warn on lookalikes", () => setS("warnLookalike", !snap!.settings.warnLookalike))}</div>
            <div class="srow"><span class="nm">Remember recent links</span>{@render toggle(snap.settings.storeRecents, "Remember recent links", () => setS("storeRecents", !snap!.settings.storeRecents))}</div>
            <div class="srow" class:off={!snap.settings.storeRecents}><span class="nm">Keep last</span>
              <div class="seg" role="group" aria-label="Recent limit">
                {#each [10, 20, 50] as n}<button aria-pressed={snap.settings.recentLimit === n} disabled={!snap.settings.storeRecents} onclick={() => setS("recentLimit", n)}>{n}</button>{/each}
              </div>
            </div>
          </div>
          <div class="sec">Danger zone</div>
          <div class="card">
            <div class="srow"><span class="nm">Reset everything<small>browsers, rules, recents and preferences</small></span>
              {#if confirmReset}<button class="btn ghost" onclick={() => (confirmReset = false)}>Cancel</button><button class="btn pri" onclick={reset}>Reset</button>
              {:else}<button class="btn danger" onclick={() => (confirmReset = true)}>Reset all…</button>{/if}
            </div>
          </div>

        {:else}
          <h1>PickMyBrowser</h1>
          <p class="sub">v{snap.version} · {snap.os} · Tauri</p>
          <p class="about">A tiny, fast browser picker. Every link you click asks which browser — or follows your rules.</p>
          <div class="row-gap">
            <button class="btn" onclick={() => api.openExternal("https://github.com/tengfone/PickMyBrowser")}>GitHub</button>
            <button class="btn" onclick={() => api.openExternal("https://github.com/tengfone/PickMyBrowser/issues/new")}>Report issue</button>
            <button class="btn" onclick={() => api.openExternal("https://buymeacoffee.com/tengfone")}>Buy me a coffee</button>
          </div>
          <div class="sec">Uninstall</div>
          <p class="hint">Run <code>PickMyBrowser --uninstall</code> to remove registrations and settings, then delete the app.</p>
        {/if}
      </main>
    </div>
  {/if}
  {#if toast}<div class="toast" role="status">{toast}</div>{/if}
{/if}

<style>
  :global(body) { background: var(--bg); }
  .shell { display: grid; grid-template-columns: 188px 1fr; height: 100vh; }
  .side { border-right: 1px solid var(--line); padding: 12px 8px; background: var(--surf); display: flex; flex-direction: column; gap: 2px; }
  .side > button { display: flex; align-items: center; height: 32px; padding: 0 12px; border-radius: 6px; color: var(--tx2); font-weight: 500; background: none; border: 0; text-align: left; cursor: pointer; }
  .side > button:hover { background: var(--surf2); color: var(--tx); }
  .side > button.on { background: var(--sel); color: var(--tx); box-shadow: inset 3px 0 0 var(--sig); }
  .ver { margin-top: auto; padding: 0 12px 4px; font: 11px/18px var(--mono); color: var(--tx3); }
  .linkish { background: none; border: 0; font: inherit; color: var(--sigtx); cursor: pointer; display: inline-flex; gap: 6px; align-items: center; padding: 0; }
  .dotw { width: 6px; height: 6px; border-radius: 50%; background: var(--sig); }
  .pane { padding: 22px 28px 40px; min-width: 0; overflow-y: auto; }
  h1 { font-size: 20px; line-height: 28px; font-weight: 700; letter-spacing: -0.01em; }
  .sub { color: var(--tx2); font-size: 13px; }
  .head { display: flex; justify-content: space-between; align-items: flex-start; gap: 16px; }
  .sec { font: 11px/16px var(--mono); color: var(--tx3); text-transform: uppercase; letter-spacing: 0.08em; margin: 20px 0 8px; }
  .card { background: var(--surf); border: 1px solid var(--line); border-radius: 8px; }
  .srow { display: flex; align-items: center; gap: 12px; min-height: 48px; padding: 6px 14px; border-top: 1px solid var(--line); }
  .srow:first-child { border-top: 0; }
  .srow.off :global(.ico), .srow.off .nm { opacity: 0.55; }
  .srow.over { box-shadow: inset 0 2px 0 var(--sig); }
  .srow[draggable="true"] { cursor: grab; }
  .grip { color: var(--tx3); font-size: 14px; }
  .nm { flex: 1; font-weight: 500; min-width: 0; }
  .nm small { display: block; font: 11px/16px var(--mono); color: var(--tx3); font-weight: 400; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .nm.url { font: 13px/18px var(--mono); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .muted { color: var(--tx3); font-weight: 400; }
  .muted code, .hint code { font: 12px var(--mono); color: var(--tx); }
  .hint { color: var(--tx3); font-size: 12px; margin-top: 10px; display: flex; gap: 6px; align-items: center; flex-wrap: wrap; }
  .mv { display: flex; gap: 2px; opacity: 0; transition: opacity 0.1s; }
  .srow:hover .mv, .srow:focus-within .mv { opacity: 1; }
  .icon { width: 24px; height: 24px; display: grid; place-items: center; border: 0; background: none; border-radius: 4px; color: var(--tx3); cursor: pointer; font-size: 12px; }
  .icon:hover:not(:disabled) { background: var(--surf2); color: var(--tx); }
  .icon:disabled { opacity: 0.3; }
  .tog { width: 34px; height: 20px; border-radius: 10px; background: var(--line2); border: 0; position: relative; flex: none; cursor: pointer; transition: background 0.12s; }
  .tog::after { content: ""; position: absolute; left: 2px; top: 2px; width: 16px; height: 16px; border-radius: 50%; background: #fff; box-shadow: 0 1px 2px rgba(0,0,0,.3); transition: transform 0.12s; }
  .tog[aria-checked="true"] { background: var(--sig); }
  .tog[aria-checked="true"]::after { transform: translateX(14px); }
  .seg { display: inline-flex; border: 1px solid var(--line2); border-radius: 6px; padding: 2px; background: var(--surf2); }
  .seg button { border: 0; background: none; font: 500 12px/20px var(--font); color: var(--tx2); padding: 2px 10px; border-radius: 4px; cursor: pointer; }
  .seg button[aria-pressed="true"] { background: var(--surf); color: var(--tx); box-shadow: 0 1px 2px rgba(0,0,0,.2); }
  .status { display: flex; align-items: center; gap: 8px; font: 12px var(--mono); color: var(--tx2); }
  .status i { width: 8px; height: 8px; border-radius: 50%; background: var(--ok); }
  .status.warn i { background: var(--sig); }
  .pat, .sel { font: 12px/20px var(--mono); background: var(--surf2); border: 1px solid var(--line2); border-radius: 4px; padding: 3px 8px; color: var(--tx); min-width: 0; }
  .pat { flex: 1.3; }
  .sel { flex: 1; font-family: var(--font); }
  .arrow { color: var(--tx3); font: 12px var(--mono); }
  .ph { width: 20px; height: 20px; border-radius: 50%; background: var(--surf2); flex: none; }
  .about { margin: 16px 0; color: var(--tx2); max-width: 46ch; }
  .row-gap { display: flex; gap: 8px; flex-wrap: wrap; }
  .ok { font: 11px var(--mono); color: var(--ok); }
  .spin { animation: spin 0.8s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
  .toast { position: fixed; bottom: 16px; left: 50%; transform: translateX(-50%); background: var(--tx); color: var(--bg); padding: 6px 14px; border-radius: 6px; font-size: 13px; font-weight: 500; }
  /* onboarding */
  .onb { display: flex; flex-direction: column; height: 100vh; background: var(--surf); }
  .onb .in { flex: 1; padding: 28px 32px; overflow-y: auto; }
  .steps { display: flex; gap: 4px; margin-bottom: 24px; }
  .steps i { height: 3px; flex: 1; background: var(--line2); border-radius: 2px; }
  .steps i.d { background: var(--sig); }
  .onb h1 { font-size: 24px; line-height: 32px; }
  .onb p { color: var(--tx2); margin: 6px 0 20px; }
  .onb .ft { display: flex; align-items: center; justify-content: space-between; padding: 12px 32px; border-top: 1px solid var(--line); background: var(--surf2); }
  .onb .ft small { font: 11px var(--mono); color: var(--tx3); }
</style>
