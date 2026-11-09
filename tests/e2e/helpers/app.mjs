// Shared E2E helpers. The app runs with PMB_E2E=1 (see wdio.conf.mjs): browsers come from
// FIXTURE_BROWSERS, launches are appended to <configDir>/launches.jsonl instead of starting
// real browsers, and set-default / launch-at-login / uninstall are disabled.
import fs from 'node:fs';
import path from 'node:path';

const E2E_IDENTIFIER = 'com.tengfone.pickmybrowser.e2e';

const chromium = (id, name, family, profiles = []) => ({
  id,
  name,
  family,
  engine: 'chromium',
  exec: `/fixture/${id}`,
  execArgs: [],
  profiles,
  enabled: true,
});
/** A fixed browser list so tests don't depend on what's installed on the runner. */
export const FIXTURE_BROWSERS = [
  chromium('chrome', 'Google Chrome', 'chrome', [
    { id: 'Default', name: 'Personal' },
    { id: 'Profile 1', name: 'Work' },
  ]),
  { ...chromium('firefox', 'Firefox', 'firefox'), engine: 'firefox' },
  chromium('edge', 'Microsoft Edge', 'edge'),
  chromium('brave', 'Brave', 'brave'),
];

// Read lazily: wdio.conf.mjs sets PMB_CONFIG_DIR after this module is imported.
const launchLog = () => path.join(process.env.PMB_CONFIG_DIR, 'launches.jsonl');

/** Launches recorded by the app since the last clearLaunches(). */
export function launches() {
  const f = launchLog();
  if (!fs.existsSync(f)) return [];
  return fs
    .readFileSync(f, 'utf8')
    .split('\n')
    .filter(Boolean)
    .map((l) => JSON.parse(l));
}
export const clearLaunches = () => fs.rmSync(launchLog(), { force: true });

export async function waitForLaunch(count = 1) {
  await browser.waitUntil(() => launches().length >= count, { timeout: 5000, timeoutMsg: `expected ${count} launch(es)` });
  return launches();
}

/** Run a command in the app via the global Tauri API (withGlobalTauri in the wdio build). */
export const invoke = (cmd, args = {}) =>
  browser.execute((c, a) => window.__TAURI__.core.invoke(c, a), cmd, args);

/** Switch the WebDriver session to a window by label ("settings" | "picker"). */
export async function use(label) {
  await browser.waitUntil(async () => (await browser.getWindowHandles()).includes(label), {
    timeout: 10000,
    timeoutMsg: `window "${label}" never appeared`,
  });
  await browser.switchToWindow(label);
  await captureErrors();
}

/** Record uncaught errors in the current page (once per page load) for expectNoPageErrors. */
async function captureErrors() {
  await browser.execute(() => {
    if (window.__e2e) return;
    const e2e = (window.__e2e = { pageErrors: [] });
    window.addEventListener('error', (ev) => e2e.pageErrors.push(ev.message || String(ev.error)));
    window.addEventListener(
      'unhandledrejection',
      (ev) => ev.defaultPrevented || e2e.pageErrors.push(String((ev.reason && ev.reason.message) || ev.reason)),
    );
  });
}

export async function expectNoPageErrors() {
  const errs = await browser.execute(() => (window.__e2e ? window.__e2e.pageErrors : null)).catch(() => null);
  if (errs && errs.length) throw new Error(`uncaught page errors: ${JSON.stringify(errs)}`);
}

/**
 * Fresh state for a spec file: reset config to defaults (rescans the fixture browsers),
 * optionally mark onboarding done, clear the launch log, and reload the settings window.
 */
export async function freshStart({ onboarded = true } = {}) {
  await use('settings');
  const id = await browser.execute(() => window.__TAURI__.app.getIdentifier());
  if (id !== E2E_IDENTIFIER) throw new Error(`refusing to reset: identifier is "${id}", expected "${E2E_IDENTIFIER}"`);
  await invoke('reset_all');
  if (onboarded) {
    const snap = await invoke('snapshot');
    await invoke('save_settings', { settings: { ...snap.settings, onboarded: true, ruleCountdownMs: 1200 } });
  }
  clearLaunches();
  await reloadCurrentWindow();
  await captureErrors();
}

/** Reload the focused window and wait until the new page has finished loading. */
async function reloadCurrentWindow() {
  // The reload is deferred so this script can return first; timeOrigin tells the old page
  // apart from the new one.
  const origin = await browser.execute(() => {
    setTimeout(() => location.reload(), 0);
    return performance.timeOrigin;
  });
  await browser.waitUntil(
    async () => {
      try {
        // A script sent while the page is mid-reload can be lost and never answer (seen on slow
        // CI runners), so each poll gets its own short timeout and is simply retried.
        return await Promise.race([
          browser.execute((o) => performance.timeOrigin !== o && document.readyState === 'complete', origin),
          new Promise((r) => setTimeout(() => r(false), 3000)),
        ]);
      } catch {
        return false;
      }
    },
    { timeout: 30000, interval: 250, timeoutMsg: 'settings window did not reload' },
  );
}

/** Send a link to the app the way the OS would, then focus the picker window. */
export async function openLink(url) {
  await use('settings');
  await invoke('pick_url', { url });
  await use('picker');
  await $('section.picker header.url').waitForDisplayed();
}

/** Visible state of a window, via the Tauri window API. */
export const isVisible = (label) =>
  browser.execute(async (l) => {
    const w = await window.__TAURI__.webviewWindow.WebviewWindow.getByLabel(l);
    return w ? w.isVisible() : false;
  }, label);
