// Settings window: onboarding, browser list, rules and preferences round-trip through the backend.
import { expect } from '@wdio/globals';
import { freshStart, invoke, FIXTURE_BROWSERS } from './helpers/app.mjs';

describe('Onboarding', () => {
  before(() => freshStart({ onboarded: false }));

  it('lists the detected browsers and finishes', async () => {
    await expect($('.onb h1')).toHaveText(`We found ${FIXTURE_BROWSERS.length} browsers`);
    await expect($$('.onb .srow')).toBeElementsArrayOfSize(FIXTURE_BROWSERS.length);
    await $('button=Continue').click();
    await expect($('.onb h1')).toHaveText('Make it your default');
    await $('button=Done').click();
    await expect($('nav.side')).toBeDisplayed();
    expect((await invoke('snapshot')).settings.onboarded).toBe(true);
  });
});

describe('Settings', () => {
  before(() => freshStart());

  it('shows the browsers in order with their hotkeys', async () => {
    await expect($('h1=Browsers')).toBeDisplayed();
    const names = await $$('main .srow .nm').map((e) => e.getText());
    FIXTURE_BROWSERS.forEach((b, i) => expect(names[i]).toContain(b.name));
  });

  it('click a key, press a letter: the browser gets its own key', async () => {
    const chip = () => $$('main .srow[data-browser-index]')[0].$('button.keychip');
    await (await chip()).click();
    await expect(await chip()).toHaveText('Press a key…');
    await browser.keys('c');
    await browser.waitUntil(async () => (await invoke('snapshot')).browsers[0].hotkey === 'c', {
      timeoutMsg: 'key was not saved',
    });
    await expect(await chip()).toHaveText('C');
    // Backspace while choosing puts it back to its number.
    await (await chip()).click();
    await browser.keys('Backspace');
    await browser.waitUntil(async () => !(await invoke('snapshot')).browsers[0].hotkey);
    await expect(await chip()).toHaveText('1');
  });

  it('dragging a browser reorders the list', async () => {
    const before = (await invoke('snapshot')).browsers.map((b) => b.id);
    const rows = await $$('main .srow[data-browser-index]');
    await rows[0].dragAndDrop(rows[2]);
    await browser.waitUntil(async () => (await invoke('snapshot')).browsers[2].id === before[0], {
      timeoutMsg: 'drag did not reorder the browsers',
    });
    // Put the order back for the tests below.
    await invoke('save_browsers', { browsers: (await invoke('snapshot')).browsers.sort((a, b) => before.indexOf(a.id) - before.indexOf(b.id)) });
    await browser.refresh();
    await expect($('h1=Browsers')).toBeDisplayed();
  });

  it('disabling a browser persists', async () => {
    await $$('main .srow')[1].$('button[role="switch"]').click();
    await browser.waitUntil(async () => (await invoke('snapshot')).browsers[1].enabled === false);
  });

  it('adds a rule', async () => {
    await $('nav.side').$('button*=Rules').click();
    await $('button*=Add rule').click();
    const input = $('main .srow input');
    await input.setValue('*.figma.com');
    await browser.keys('Tab');
    await browser.waitUntil(async () => (await invoke('snapshot')).rules[0]?.pattern === '*.figma.com', {
      timeoutMsg: 'rule not saved',
    });
  });

  it('changes a preference', async () => {
    await $('nav.side').$('button*=General').click();
    await $('button[aria-label="Strip tracking"]').click();
    await browser.waitUntil(async () => (await invoke('snapshot')).settings.stripTracking === false);
    await $('button[aria-label="Strip tracking"]').click();
    await browser.waitUntil(async () => (await invoke('snapshot')).settings.stripTracking === true);
  });

  it('cannot touch the real system from the E2E build', async () => {
    await expect(invoke('set_default')).rejects.toThrow(/disabled in E2E/);
    await expect(invoke('uninstall_app')).rejects.toThrow(/disabled in E2E/);
  });
});
