// Rules (auto-open after a countdown) and the type-to-filter list (more than 9 browsers).
import { expect } from '@wdio/globals';
import { freshStart, openLink, launches, clearLaunches, waitForLaunch, invoke, use, FIXTURE_BROWSERS } from './helpers/app.mjs';

const setRules = (rules) => invoke('save_rules', { rules });

describe('Rules', () => {
  before(async () => {
    await freshStart();
    await setRules([
      { pattern: '*.figma.com', browserId: 'chrome', profileId: 'Profile 1', enabled: true },
      { pattern: 'github.com/acme/*', browserId: 'firefox', profileId: null, enabled: true },
    ]);
  });
  beforeEach(() => clearLaunches());

  it('a matching link opens automatically after the countdown', async () => {
    await openLink('https://www.figma.com/file/abc');
    await expect($('.rule .t')).toHaveText(expect.stringContaining('Opening in Google Chrome · Work'));
    const [l] = await waitForLaunch();
    expect(l).toMatchObject({ browser: 'chrome', profile: 'Profile 1', url: 'https://www.figma.com/file/abc' });
  });

  it('Space cancels the countdown and shows the full list', async () => {
    await openLink('https://github.com/acme/repo');
    await browser.keys(' ');
    await expect($('ul.list')).toBeDisplayed();
    await browser.pause(1800); // longer than the countdown
    expect(launches()).toHaveLength(0);
    await browser.keys('4');
    expect((await waitForLaunch())[0].browser).toBe('brave');
  });

  it('a non-matching path shows the picker', async () => {
    await openLink('https://github.com/other/repo');
    await expect($('ul.list')).toBeDisplayed();
    await browser.keys('Escape');
  });

  it('a lookalike link never auto-opens', async () => {
    await setRules([{ pattern: 'paypa1.com', browserId: 'chrome', profileId: null, enabled: true }]);
    await openLink('https://paypa1.com/');
    await expect($('.warn')).toBeDisplayed();
    await browser.pause(1800);
    expect(launches()).toHaveLength(0);
    await browser.keys('Escape');
  });

  it('countdown 0 opens without showing the picker', async () => {
    await setRules([{ pattern: 'example.org', browserId: 'edge', profileId: null, enabled: true }]);
    await use('settings');
    const s = (await invoke('snapshot')).settings;
    await invoke('save_settings', { settings: { ...s, ruleCountdownMs: 0 } });
    await invoke('pick_url', { url: 'https://example.org/' });
    expect((await waitForLaunch())[0].browser).toBe('edge');
    await invoke('save_settings', { settings: { ...s, ruleCountdownMs: 1200 } });
  });

  it("a rule whose browser won't start falls back to the picker", async () => {
    await use('settings');
    const snap = await invoke('snapshot');
    const browsers = snap.browsers.map((b) => (b.id === 'edge' ? { ...b, exec: '/fixture/missing' } : b));
    await invoke('save_browsers', { browsers });
    await setRules([{ pattern: 'example.org', browserId: 'edge', profileId: null, enabled: true }]);
    await invoke('save_settings', { settings: { ...snap.settings, ruleCountdownMs: 0 } });
    await openLink('https://example.org/gone');
    await expect($('.notice')).toHaveText(expect.stringContaining("couldn't be started"));
    await expect($('ul.list')).toBeDisplayed();
    await browser.keys('2'); // Firefox
    expect((await waitForLaunch())[0].browser).toBe('firefox');
    await use('settings');
    await invoke('save_browsers', { browsers: snap.browsers });
    await invoke('save_settings', { settings: snap.settings });
  });

  it('Enter on a focused Cancel button cancels instead of opening', async () => {
    await setRules([{ pattern: '*.figma.com', browserId: 'chrome', profileId: null, enabled: true }]);
    await use('settings');
    const s = (await invoke('snapshot')).settings;
    await invoke('save_settings', { settings: { ...s, ruleCountdownMs: 2000 } });
    await openLink('https://www.figma.com/cancel');
    await browser.execute(() => document.querySelector('.rule .act .ghost').focus());
    await browser.keys('Enter');
    const early = launches().length;
    await browser.pause(2500); // longer than the countdown
    expect({ early, launched: launches().map((l) => l.url) }).toEqual({ early: 0, launched: [] });
    await use('settings');
    await invoke('save_settings', { settings: s });
  });
});

describe('Filter (more than 9 browsers)', () => {
  before(async () => {
    await freshStart();
    const extra = Array.from({ length: 8 }, (_, i) => ({
      ...FIXTURE_BROWSERS[3],
      id: `extra${i}`,
      name: `Extra Browser ${i}`,
      family: 'other',
      engine: 'other',
    }));
    await invoke('save_browsers', { browsers: [...FIXTURE_BROWSERS, ...extra] });
  });
  beforeEach(() => clearLaunches());

  it('typing filters the list and Enter opens the first match', async () => {
    await openLink('https://example.com/filter');
    await expect($('.filter')).toBeDisplayed();
    await browser.keys(['f', 'i', 'r']);
    await expect($('.filter .n')).toHaveText('1 of 12');
    await browser.keys('Enter');
    expect((await waitForLaunch())[0].browser).toBe('firefox');
  });
});
