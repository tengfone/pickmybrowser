// Backdrop for the README demo: a fake desktop with a chat window full of links and a caption
// pill. Rendered to PNG with headless Edge/Chrome. Exports the link positions so the recorder
// can move the (drawn) cursor onto them.
import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { pathToFileURL } from 'node:url';

export const W = 1280;
export const H = 800;

// Chat window geometry (CSS px == output px).
const CHAT = { x: 90, y: 70, w: 700, h: 640 };
export const LINKS = [
  { who: 'Maya', text: 'PR is up, can you review?', url: 'https://github.com/tengfone/pickmybrowser/pull/42' },
  { who: 'Dev', text: 'Docs for the new API', url: 'https://developer.mozilla.org/docs/Web/API?utm_source=chat&fbclid=x' },
  { who: 'Sam', text: 'Standup notes', url: 'https://docs.google.com/document/d/1x2y3z' },
  { who: 'Priya', text: 'Updated designs 🎨', url: 'https://www.figma.com/file/k9Ad/onboarding' },
];
const ROW_H = 118;
const ROW_TOP = CHAT.y + 112;
/** Where the cursor should click for link i (output px). */
export const linkPoint = (i) => ({ x: CHAT.x + 96 + 150, y: ROW_TOP + i * ROW_H + 58 });

const browserExe = () =>
  [
    'C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe',
    'C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe',
    'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe',
    '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    '/usr/bin/google-chrome',
    '/usr/bin/chromium',
  ].find((p) => fs.existsSync(p));

function html(root, { caption, hover = -1 }) {
  const font = (p) => pathToFileURL(path.join(root, 'node_modules', '@fontsource', p)).href;
  const msgs = LINKS.map(
    (l, i) => `
    <div class="msg" style="top:${ROW_TOP - CHAT.y + i * ROW_H}px">
      <div class="av" style="background:hsl(${i * 83 + 15} 45% 45%)">${l.who[0]}</div>
      <div><div class="who">${l.who} <span>${9 + i}:${String(12 + i * 7).padStart(2, '0')} AM</span></div>
      <div class="tx">${l.text}</div>
      <div class="ln${i === hover ? ' hv' : ''}">${l.url.replace(/^https:\/\//, '').replace(/\?.*/, '')}</div></div>
    </div>`,
  ).join('');
  return `<!doctype html><html><head><meta charset="utf-8"><style>
@font-face{font-family:Plex;font-weight:400;src:url(${font('ibm-plex-sans/files/ibm-plex-sans-latin-400-normal.woff2')})}
@font-face{font-family:Plex;font-weight:600;src:url(${font('ibm-plex-sans/files/ibm-plex-sans-latin-600-normal.woff2')})}
@font-face{font-family:Mono;src:url(${font('jetbrains-mono/files/jetbrains-mono-latin-400-normal.woff2')})}
*{box-sizing:border-box;margin:0}
body{width:${W}px;height:${H}px;overflow:hidden;font-family:Plex,sans-serif;
  background:radial-gradient(1200px 700px at 78% 18%,#3a2a26 0%,transparent 60%),
             radial-gradient(900px 600px at 10% 90%,#1d2a33 0%,transparent 60%),#141517}
.chat{position:absolute;left:${CHAT.x}px;top:${CHAT.y}px;width:${CHAT.w}px;height:${CHAT.h}px;background:#f4f3f0;
  border-radius:12px;overflow:hidden;box-shadow:0 30px 80px rgba(0,0,0,.55),0 0 0 1px rgba(255,255,255,.06)}
.bar{height:44px;background:#e9e7e2;display:flex;align-items:center;padding:0 16px;gap:8px;border-bottom:1px solid #dcd9d3}
.bar i{width:12px;height:12px;border-radius:50%;background:#cfccc5}
.bar b{margin-left:14px;font-weight:600;color:#2a2b2e;font-size:14px}
.ch{padding:14px 22px;border-bottom:1px solid #e3e1dc;color:#55575c;font-size:14px}
.ch b{color:#1f2023}
.msg{position:absolute;left:22px;right:22px;display:flex;gap:14px}
.av{width:40px;height:40px;border-radius:9px;color:#fff;display:grid;place-items:center;font-weight:600;flex:none}
.who{font-weight:600;color:#1f2023;font-size:15px}.who span{font-weight:400;color:#8a8c91;font-size:12px;margin-left:6px}
.tx{color:#3a3c40;font-size:15px;margin:3px 0 6px}
.ln{font-family:Mono,monospace;font-size:13.5px;color:#1f5fbf;text-decoration:underline;text-underline-offset:3px;
  padding:3px 6px;margin-left:-6px;border-radius:5px;display:inline-block}
.ln.hv{background:#dfe8f7}
.cap{position:absolute;left:0;right:0;bottom:34px;display:flex;justify-content:center}
.cap div{background:rgba(23,24,26,.88);color:#ecedee;border:1px solid #3f4248;border-radius:999px;
  padding:11px 22px;font-size:17px;letter-spacing:.1px;backdrop-filter:blur(8px)}
.cap kbd{font-family:Mono,monospace;font-size:14px;background:#2a2c30;border:1px solid #4a4d54;border-bottom-width:2px;
  border-radius:5px;padding:1px 7px;margin:0 2px;color:#fff}
.cap em{font-style:normal;color:#ff6a4a}
.brand{position:absolute;right:40px;top:30px;color:#ecedee;font-weight:600;font-size:18px;display:flex;gap:10px;align-items:center;opacity:.9}
.brand small{display:block;font-weight:400;color:#9a9ca1;font-size:13px}
</style></head><body>
<div class="brand"><img src="${pathToFileURL(path.join(root, 'design', 'icons', 'pmb.svg')).href}" width="40"><div>PickMyBrowser<small>one link, your choice of browser</small></div></div>
<div class="chat"><div class="bar"><i></i><i></i><i></i><b>Team chat</b></div>
<div class="ch"><b># launch</b> &nbsp;·&nbsp; 6 members</div>${msgs}</div>
${caption ? `<div class="cap"><div>${caption}</div></div>` : ''}
</body></html>`;
}

/** Render one backdrop variant to PNG; returns the file path. */
export function renderBackdrop(root, dir, name, opts) {
  const exe = browserExe();
  if (!exe) throw new Error('Edge or Chrome is needed to render the demo backdrop');
  dir = path.resolve(dir);
  const page = path.join(dir, `${name}.html`);
  const png = path.join(dir, `${name}.png`);
  fs.writeFileSync(page, html(root, opts));
  execFileSync(exe, [
    '--headless',
    '--disable-gpu',
    '--hide-scrollbars',
    '--force-device-scale-factor=1',
    `--window-size=${W},${H}`,
    '--virtual-time-budget=2000',
    '--allow-file-access-from-files',
    `--user-data-dir=${path.join(dir, 'edge-profile')}`,
    `--screenshot=${png}`,
    pathToFileURL(page).href,
  ], { stdio: 'ignore' });
  if (!fs.existsSync(png)) throw new Error(`backdrop render failed: ${name}`);
  return png;
}
