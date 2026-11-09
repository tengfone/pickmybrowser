// WebdriverIO config for the Tauri E2E suite.
// The app under test is the `--features wdio` build (npm run build:e2e), which embeds the
// W3C WebDriver server (tauri-plugin-wdio-webdriver), so no tauri-driver/msedgedriver.
// PMB_E2E=1 + PMB_CONFIG_DIR: no tray, no single-instance lock, no real browser launches
// (logged to launches.jsonl instead), a fixed browser list, and a throwaway config folder.
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { expectNoPageErrors, FIXTURE_BROWSERS } from './tests/e2e/helpers/app.mjs';

const root = path.dirname(fileURLToPath(import.meta.url));
const appBinaryPath = path.join(
  root,
  'src-tauri',
  'target',
  'wdio',
  'release',
  process.platform === 'win32' ? 'pickmybrowser.exe' : 'pickmybrowser',
);

// One throwaway config folder per run, seeded with the fixture browsers. This file is also
// loaded by every worker process; they inherit the launcher's folder through the env var.
export const configDir = process.env.PMB_CONFIG_DIR || fs.mkdtempSync(path.join(os.tmpdir(), 'pmb-e2e-'));
fs.writeFileSync(path.join(configDir, 'fixture-browsers.json'), JSON.stringify(FIXTURE_BROWSERS));
process.env.PMB_CONFIG_DIR = configDir;

export const config = {
  runner: 'local',
  specs: ['./tests/e2e/**/*.spec.mjs'],
  maxInstances: 1,
  capabilities: [
    {
      browserName: 'tauri',
      'tauri:options': { application: appBinaryPath },
    },
  ],
  services: [
    [
      '@wdio/tauri-service',
      {
        appBinaryPath,
        driverProvider: 'embedded',
        env: { PMB_E2E: '1', PMB_CONFIG_DIR: configDir },
        captureFrontendLogs: true,
        frontendLogLevel: 'error',
      },
    ],
  ],
  strictSelectors: false,
  logLevel: 'warn',
  bail: 0,
  waitforTimeout: 10000,
  waitforInterval: 100,
  connectionRetryTimeout: 30000,
  connectionRetryCount: 2,
  framework: 'mocha',
  mochaOpts: { ui: 'bdd', timeout: 60000 },
  reporters: ['spec'],
  outputDir: path.join(root, 'test-results', 'logs'),
  // Every passing test must leave no uncaught page errors; on failure keep a screenshot.
  afterTest: async (test, _context, { passed }) => {
    if (passed) {
      await expectNoPageErrors();
      return;
    }
    const dir = path.join(root, 'test-results', 'screenshots');
    fs.mkdirSync(dir, { recursive: true });
    const name = `${test.parent} ${test.title}`.replace(/[^\w.-]+/g, '_').slice(0, 150);
    await browser.saveScreenshot(path.join(dir, `${name}.png`)).catch((e) => console.warn('screenshot failed:', e.message));
  },
  onComplete: () => fs.rmSync(configDir, { recursive: true, force: true }),
};
