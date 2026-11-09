// Prints app binary, installer and frontend sizes. Informational only: always exits 0.
//   node scripts/report-size.cjs
const fs = require('fs');
const path = require('path');

const root = path.join(__dirname, '..');
const targets = path.join(root, 'src-tauri', 'target');
const mb = (n) => (n / 1048576).toFixed(2) + ' MiB';
const walk = (d) =>
  fs.existsSync(d)
    ? fs.readdirSync(d, { withFileTypes: true }).flatMap((e) => (e.isDirectory() ? walk(path.join(d, e.name)) : [path.join(d, e.name)]))
    : [];
const size = (f) => fs.statSync(f).size;

const rows = [];
const rels = [path.join(targets, 'release'), ...['aarch64-apple-darwin', 'x86_64-apple-darwin'].map((t) => path.join(targets, t, 'release'))];
for (const rel of rels) {
  for (const name of ['pickmybrowser.exe', 'pickmybrowser']) {
    const f = path.join(rel, name);
    if (fs.existsSync(f)) rows.push(['binary', path.relative(targets, f), size(f)]);
  }
  for (const f of walk(path.join(rel, 'bundle')).filter((f) => /\.(exe|dmg|AppImage|deb|rpm)$/.test(f)))
    rows.push(['installer', path.relative(targets, f), size(f)]);
}
const front = walk(path.join(root, 'build'));
if (front.length) rows.push(['frontend', 'build/ (total)', front.reduce((s, f) => s + size(f), 0)]);
if (!rows.length) console.log('report-size: nothing built yet');
for (const [kind, name, n] of rows) console.log(`${kind.padEnd(10)} ${mb(n).padStart(10)}  ${name}`);
