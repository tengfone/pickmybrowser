#!/usr/bin/env node
// Builds latest.json for the Tauri updater from the signed release assets.
// Usage: node scripts/updater-manifest.cjs <release-dir> <version> <tag>
// Asset names must match what the release workflow's "Collect release asset" step produces.
const fs = require('fs');
const path = require('path');

const [dir, version, tag] = process.argv.slice(2);
if (!dir || !version || !tag) {
  console.error('usage: updater-manifest.cjs <release-dir> <version> <tag>');
  process.exit(1);
}
const base = `https://github.com/tengfone/pickmybrowser/releases/download/${tag}`;
const assets = {
  'windows-x86_64': `PickMyBrowser-Setup-${version}.exe`,
  'darwin-aarch64': `PickMyBrowser-${version}-mac-arm64.app.tar.gz`,
  'darwin-x86_64': `PickMyBrowser-${version}-mac-x64.app.tar.gz`,
  'linux-x86_64': `PickMyBrowser-${version}.AppImage`,
};

const platforms = {};
for (const [platform, file] of Object.entries(assets)) {
  const sig = path.join(dir, `${file}.sig`);
  if (!fs.existsSync(sig)) {
    console.error(`missing ${sig}`);
    process.exit(1);
  }
  platforms[platform] = { signature: fs.readFileSync(sig, 'utf8').trim(), url: `${base}/${file}` };
}

const manifest = {
  version,
  notes: `See https://github.com/tengfone/pickmybrowser/releases/tag/${tag}`,
  pub_date: new Date().toISOString(),
  platforms,
};
process.stdout.write(JSON.stringify(manifest, null, 2) + '\n');
