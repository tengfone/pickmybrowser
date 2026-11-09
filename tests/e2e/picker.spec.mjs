// Picker: a link arrives, the user picks a browser by hotkey, click or profile.
// Launches are recorded by the E2E build instead of starting real browsers.
import { expect } from '@wdio/globals';
import { freshStart, openLink, launches, clearLaunches, waitForLaunch, invoke, use, isVisible } from './helpers/app.mjs';

describe('Picker', () => {
  before(() => freshStart());
  beforeEach(() => clearLaunches());

  it('shows the link and every enabled browser', async () => {
    await openLink('https://example.com/docs/page');
    await expect($('header.url b')).toHaveText('example.com');
    await expect($$('ul.list li.row')).toBeElementsArrayOfSize(4);
    expect(await isVisible('picker')).toBe(true);
    await browser.keys('Escape');
  });

  it('number key opens that browser and hides the picker', async () => {
    await openLink('https://example.com/a');
    await browser.keys('2');
    const [l] = await waitForLaunch();
    expect(l).toMatchObject({ browser: 'firefox', url: 'https://example.com/a', private: false });
    await browser.waitUntil(async () => !(await isVisible('picker')), { timeoutMsg: 'picker stayed open' });
  });

  it('Shift + number opens a private window', async () => {
    await openLink('https://example.com/b');
    await browser.keys(['Shift', '1']);
    const [l] = await waitForLaunch();
    expect(l).toMatchObject({ browser: 'chrome', private: true });
    expect(l.args).toContain('--incognito');
  });

  it('arrow keys + Enter', async () => {
    await openLink('https://example.com/c');
    await browser.keys(['ArrowDown', 'ArrowDown', 'Enter']);
    const [l] = await waitForLaunch();
    expect(l.browser).toBe('edge');
  });

  it('clicking a row opens it', async () => {
    await openLink('https://example.com/d');
    await $$('ul.list li.row')[3].click();
    const [l] = await waitForLaunch();
    expect(l.browser).toBe('brave');
  });

  it('opens a profile with → then its letter', async () => {
    await openLink('https://example.com/e');
    await browser.keys('ArrowRight');
    await expect($$('ul.list li.row.sub')).toBeElementsArrayOfSize(2);
    await browser.keys('w');
    const [l] = await waitForLaunch();
    expect(l).toMatchObject({ browser: 'chrome', profile: 'Profile 1' });
    expect(l.args).toContain('--profile-directory=Profile 1');
  });

  it('opening the profile list keeps the window where it is', async () => {
    await openLink('https://example.com/stay');
    const position = () =>
      browser.execute(async () => {
        const w = await window.__TAURI__.webviewWindow.WebviewWindow.getByLabel('picker');
        const p = await w.outerPosition();
        return [p.x, p.y];
      });
    // Put the window somewhere with room to grow (near the top-left of its screen), so the
    // only thing that could move it is the bug this test guards against. Near a screen edge
    // the picker is allowed to shift just enough to stay visible.
    await browser.execute(async () => {
      const { WebviewWindow } = window.__TAURI__.webviewWindow;
      const { PhysicalPosition } = window.__TAURI__.dpi;
      const w = await WebviewWindow.getByLabel('picker');
      const m = await window.__TAURI__.window.currentMonitor();
      const area = m.workArea ?? { position: m.position };
      await w.setPosition(new PhysicalPosition(area.position.x + 120, area.position.y + 90));
    });
    await browser.pause(200);
    const before = await position();
    await $('ul.list li.row button.caret').click();
    await expect($$('ul.list li.row.sub')).toBeElementsArrayOfSize(2);
    await browser.pause(300);
    expect(await position()).toEqual(before);
    await browser.keys('Escape');
    await browser.keys('Escape');
  });

  it('Escape closes without opening anything', async () => {
    await openLink('https://example.com/f');
    await browser.keys('Escape');
    await browser.waitUntil(async () => !(await isVisible('picker')));
    await browser.pause(300);
    expect(launches()).toHaveLength(0);
  });

  it('strips tracking parameters', async () => {
    await openLink('https://example.com/g?utm_source=x&fbclid=y&id=7');
    await expect($('.chip.info')).toHaveText(expect.stringContaining('2 trackers'));
    await browser.keys('1');
    const [l] = await waitForLaunch();
    expect(l.url).toBe('https://example.com/g?id=7');
  });

  it('warns about lookalike domains', async () => {
    await openLink('https://paypa1.com/login');
    await expect($('.warn')).toHaveText(expect.stringContaining('paypal.com'));
    await browser.keys('Escape');
  });

  it('records the link in recents with the browser used', async () => {
    await openLink('https://example.com/recent');
    await browser.keys('3');
    await waitForLaunch();
    await use('settings');
    const r = (await invoke('snapshot')).recents[0];
    expect(r).toMatchObject({ url: 'https://example.com/recent', browserId: 'edge' });
  });

  it('a custom key opens its browser; the others keep their numbers', async () => {
    await use('settings');
    const browsers = (await invoke('snapshot')).browsers;
    browsers.find((b) => b.id === 'firefox').hotkey = 'f';
    await invoke('save_browsers', { browsers });
    // Chrome 1, Firefox F, Edge 2, Brave 3.
    await openLink('https://example.com/key');
    await expect($$('ul.list li.row .kbd')).toHaveText(['1', 'F', '2', '3']);
    await browser.keys('f');
    const [first] = await waitForLaunch();
    expect(first.browser).toBe('firefox');
    clearLaunches();
    await openLink('https://example.com/key2');
    await browser.keys('2');
    const [second] = await waitForLaunch();
    expect(second.browser).toBe('edge');
    // Clean up for the other specs.
    await use('settings');
    browsers.find((b) => b.id === 'firefox').hotkey = null;
    await invoke('save_browsers', { browsers });
  });
});
