// Records the README demo GIF (assets/demo.gif) from the e2e test build.
//   npm run build:e2e && npm run demo
// Requires ffmpeg on PATH and Edge/Chrome (backdrop rendering). Runs as a WDIO spec.
// Real app screenshots (picker / settings) are composited onto a fake desktop backdrop
// with a drawn cursor, at a fixed frame rate.
import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { PNG } from 'pngjs';
import { freshStart, invoke, openLink, use } from '../tests/e2e/helpers/app.mjs';
import { W, H, LINKS, linkPoint, renderBackdrop } from './demo-backdrop.mjs';

const root = path.join(path.dirname(fileURLToPath(import.meta.url)), '..');
const dir = path.join(root, '.demo-frames');
const framesDir = path.join(dir, 'frames');
const out = path.join(root, 'assets', 'demo.gif');
const FPS = 15;

/* ---------- image helpers ---------- */
const decode = (b64) => PNG.sync.read(Buffer.from(b64, 'base64'));
const bgCache = new Map();
function backdrop(caption, hover = -1) {
  const key = `${caption}|${hover}`;
  if (!bgCache.has(key)) {
    const f = renderBackdrop(root, dir, `bg${bgCache.size}`, { caption, hover });
    bgCache.set(key, PNG.sync.read(fs.readFileSync(f)));
  }
  return bgCache.get(key);
}

function scale(img, s) {
  if (s === 1) return img;
  const w = Math.round(img.width * s), h = Math.round(img.height * s);
  const o = new PNG({ width: w, height: h });
  const r = 1 / s;
  for (let y = 0; y < h; y++)
    for (let x = 0; x < w; x++) {
      // box filter over the source area
      const x0 = Math.floor(x * r), y0 = Math.floor(y * r);
      const x1 = Math.min(img.width, Math.ceil((x + 1) * r)), y1 = Math.min(img.height, Math.ceil((y + 1) * r));
      let a = 0, b = 0, c = 0, n = 0;
      for (let yy = y0; yy < y1; yy++)
        for (let xx = x0; xx < x1; xx++) {
          const i = (yy * img.width + xx) * 4;
          a += img.data[i]; b += img.data[i + 1]; c += img.data[i + 2]; n++;
        }
      const j = (y * w + x) * 4;
      o.data[j] = a / n; o.data[j + 1] = b / n; o.data[j + 2] = c / n; o.data[j + 3] = 255;
    }
  return o;
}

function blendPx(d, i, r, g, b, a) {
  d[i] = d[i] * (1 - a) + r * a;
  d[i + 1] = d[i + 1] * (1 - a) + g * a;
  d[i + 2] = d[i + 2] * (1 - a) + b * a;
}

/** Paste a window screenshot with rounded corners and a soft drop shadow. */
function pasteWindow(frame, img, X, Y, alpha = 1, rad = 10) {
  const d = frame.data, S = 28;
  for (let y = Y - S; y < Y + img.height + S + 12; y++) {
    if (y < 0 || y >= H) continue;
    for (let x = X - S; x < X + img.width + S; x++) {
      if (x < 0 || x >= W) continue;
      const lx = x - X, ly = y - Y;
      const i = (y * W + x) * 4;
      // distance to the rounded rect (shadow offset 10px down)
      const sdist = rrDist(lx, ly - 10, img.width, img.height, rad);
      if (sdist > 0 && sdist < S) blendPx(d, i, 0, 0, 0, 0.45 * alpha * (1 - sdist / S) ** 2);
      const dist = rrDist(lx, ly, img.width, img.height, rad);
      if (dist <= 0.5 && lx >= 0 && ly >= 0 && lx < img.width && ly < img.height) {
        const a = alpha * Math.min(1, 0.5 - dist);
        const k = (ly * img.width + lx) * 4;
        blendPx(d, i, img.data[k], img.data[k + 1], img.data[k + 2], a);
      }
    }
  }
}
function rrDist(x, y, w, h, r) {
  const qx = Math.abs(x + 0.5 - w / 2) - (w / 2 - r), qy = Math.abs(y + 0.5 - h / 2) - (h / 2 - r);
  return Math.hypot(Math.max(qx, 0), Math.max(qy, 0)) + Math.min(Math.max(qx, qy), 0) - r;
}

// Classic arrow cursor, tip at (0,0).
const ARROW = [[0, 0], [0, 17], [4, 13], [7, 20], [10, 19], [7, 12], [12, 12]];
function inPoly(px, py, poly) {
  let c = false;
  for (let i = 0, j = poly.length - 1; i < poly.length; j = i++) {
    const [xi, yi] = poly[i], [xj, yj] = poly[j];
    if (yi > py !== yj > py && px < ((xj - xi) * (py - yi)) / (yj - yi) + xi) c = !c;
  }
  return c;
}
function drawCursor(frame, cx, cy, pressed) {
  const s = 1.25;
  for (let y = -3; y < 28; y++)
    for (let x = -3; x < 20; x++) {
      const X = Math.round(cx + x), Y = Math.round(cy + y);
      if (X < 0 || Y < 0 || X >= W || Y >= H) continue;
      const u = x / s, v = y / s;
      const inner = inPoly(u, v, ARROW);
      const outer = inner || [[-1, 0], [1, 0], [0, -1], [0, 1], [-1, -1], [1, 1]].some(([a, b]) => inPoly(u + a / s, v + b / s, ARROW));
      const i = (Y * W + X) * 4;
      if (inner) blendPx(frame.data, i, pressed ? 200 : 255, pressed ? 56 : 255, pressed ? 28 : 255, 1);
      else if (outer) blendPx(frame.data, i, 0, 0, 0, 1);
    }
}

/* ---------- timeline ---------- */
let n = 0;
let cursor = { x: 1000, y: 620 };
let state = { caption: '', hover: -1, win: null, pressed: false };

function emit(count = 1) {
  const f = new PNG({ width: W, height: H });
  backdrop(state.caption, state.hover).data.copy(f.data);
  if (state.win) pasteWindow(f, state.win.img, state.win.x, state.win.y, state.win.alpha ?? 1);
  drawCursor(f, cursor.x, cursor.y, state.pressed);
  const buf = PNG.sync.write(f);
  for (let k = 0; k < count; k++) fs.writeFileSync(path.join(framesDir, `f${String(n++).padStart(5, '0')}.png`), buf);
}
const hold = (sec) => emit(Math.round(sec * FPS));
function moveTo(x, y, sec = 0.6) {
  const from = { ...cursor }, steps = Math.round(sec * FPS);
  for (let s = 1; s <= steps; s++) {
    const t = s / steps, e = t < 0.5 ? 2 * t * t : 1 - (-2 * t + 2) ** 2 / 2;
    cursor = { x: from.x + (x - from.x) * e, y: from.y + (y - from.y) * e };
    emit();
  }
}
function click() {
  state.pressed = true; emit(2); state.pressed = false;
}
function fadeOut(sec = 0.25) {
  const steps = Math.round(sec * FPS);
  for (let s = steps - 1; s >= 0; s--) { state.win.alpha = s / steps; emit(); }
  state.win = null;
}

/* ---------- app helpers ---------- */
function crop(img, rect) {
  const w = Math.min(rect.w, img.width - rect.x), h = Math.min(rect.h, img.height - rect.y);
  const o = new PNG({ width: w, height: h });
  for (let y = 0; y < h; y++) {
    const start = ((y + rect.y) * img.width + rect.x) * 4;
    img.data.copy(o.data, y * w * 4, start, start + w * 4);
  }
  return o;
}
/** Screenshot of the picker cropped to its card (the window can be taller than the card). */
async function grabPickerCard() {
  const r = await browser.execute(() => {
    const c = document.querySelector('section.picker');
    if (!c) return null;
    const b = c.getBoundingClientRect(), d = devicePixelRatio;
    return { x: Math.round(b.left * d), y: Math.round(b.top * d), w: Math.round(b.width * d), h: Math.round(b.height * d) };
  });
  const img = decode(await browser.takeScreenshot());
  return r ? crop(img, r) : img;
}

/** Wait for the picker to repaint, then grab it. */
async function shot() {
  await browser.pause(250);
  return grabPickerCard();
}

/** Place the picker just below-right of the link that opened it, kept on screen. */
const pickerPos = (i) => {
  const p = linkPoint(i);
  return { x: p.x + 40, y: Math.min(p.y + 22, H - 330) };
};

async function showPicker(i, img) {
  state.win = { img: img ?? (await shot()), ...pickerPos(i), alpha: 1 };
}

/* ---------- scene building blocks ---------- */
/** Move the cursor onto link i, hover it briefly, click, and open that link in the app. */
async function clickLink(i, moveSec) {
  const p = linkPoint(i);
  moveTo(p.x, p.y, moveSec);
  state.hover = i;
  emit(4);
  click();
  await openLink(LINKS[i].url);
}

/** Press keys in the picker, then show the result for `sec` seconds. */
async function pressAndShow(i, keys, sec) {
  await browser.keys(keys);
  await showPicker(i);
  hold(sec);
}

function closePicker() {
  fadeOut();
  state.hover = -1;
  hold(0.4);
}

/* ---------- scenes ---------- */
async function sceneNumberKey() {
  state.caption = 'Click any link. <em>Pick a browser</em> with <kbd>1</kbd>-<kbd>9</kbd>';
  hold(0.8);
  await clickLink(0, 0.8);
  await showPicker(0);
  hold(1.0);
  await pressAndShow(0, 'ArrowDown', 0.9); // highlight moves, as if the user pressed 2
  await browser.keys('Enter');
  closePicker();
}

async function scenePrivateWindow() {
  state.caption = 'Hold <kbd>Shift</kbd> for a <em>private window</em>. Trackers are stripped.';
  await clickLink(1);
  await showPicker(1);
  hold(1.2);
  const SHIFT = '\uE008';
  await browser.action('key').down(SHIFT).perform(true);
  await showPicker(1);
  hold(1.4);
  await browser.action('key').up(SHIFT).perform();
  await browser.keys('Escape');
  closePicker();
}

async function sceneProfiles() {
  state.caption = 'Press <kbd>→</kbd> to open a <em>specific profile</em>';
  await clickLink(2);
  await showPicker(2);
  hold(1.0);
  await pressAndShow(2, 'ArrowRight', 1.2);
  await pressAndShow(2, 'ArrowDown', 0.9);
  await browser.keys('Enter');
  closePicker();
}

async function sceneRuleCountdown() {
  state.caption = '<em>Rules</em> open known sites automatically. <kbd>Space</kbd> to cancel.';
  await clickLink(3);
  // Screenshots are slower than the frame rate, so grab as many as possible during the
  // countdown, then use the nearest one for each output frame.
  const SECONDS = 2.1;
  const t0 = Date.now();
  const shots = [];
  while (Date.now() - t0 < SECONDS * 1000) {
    try {
      shots.push({ t: Date.now() - t0, img: await grabPickerCard() });
    } catch {
      break; // the picker closed when the rule fired
    }
  }
  for (let k = 0; k < Math.round(SECONDS * FPS); k++) {
    const t = (k * 1000) / FPS;
    const nearest = shots.reduce((a, b) => (Math.abs(b.t - t) < Math.abs(a.t - t) ? b : a));
    await showPicker(3, nearest.img);
    emit();
  }
  closePicker();
}

async function sceneSettings() {
  state.caption = 'Browsers, profiles, rules and recents in <em>Settings</em>';
  await use('settings');
  await $('nav.side').$('button*=Rules').click();
  await browser.pause(250);
  let img = decode(await browser.takeScreenshot());
  img = scale(img, Math.min(1, 1000 / img.width, 600 / img.height));
  moveTo(1100, 700, 0.4);
  // Centred in the area above the caption pill.
  state.win = { img, x: Math.round((W - img.width) / 2), y: Math.round((H - 70 - img.height) / 2), alpha: 1 };
  hold(2.4);
}

/** Two-pass GIF encode: build a palette from the frames, then dither with it. */
function encodeGif() {
  fs.mkdirSync(path.dirname(out), { recursive: true });
  const palette = path.join(dir, 'palette.png');
  const input = ['-framerate', String(FPS), '-i', path.join(framesDir, 'f%05d.png')];
  const resize = 'scale=960:-1:flags=lanczos';
  execFileSync('ffmpeg', ['-y', '-v', 'error', ...input, '-vf', `${resize},palettegen=stats_mode=diff`, palette]);
  execFileSync('ffmpeg', [
    '-y', '-v', 'error', ...input, '-i', palette,
    '-lavfi', `${resize}[x];[x][1:v]paletteuse=dither=sierra2_4a:diff_mode=rectangle`,
    '-loop', '0', out,
  ]);
  console.log(`DEMO ${out} ${n} frames ${(fs.statSync(out).size / 1048576).toFixed(2)} MiB`);
}

describe('demo', () => {
  it('records', async () => {
    fs.rmSync(framesDir, { recursive: true, force: true });
    fs.mkdirSync(framesDir, { recursive: true });
    await freshStart();
    const snap = await invoke('snapshot');
    await invoke('save_settings', { settings: { ...snap.settings, ruleCountdownMs: 2200 } });
    await invoke('save_rules', {
      rules: [{ pattern: '*.figma.com', browserId: 'chrome', profileId: 'Profile 1', enabled: true }],
    });

    await sceneNumberKey();
    await scenePrivateWindow();
    await sceneProfiles();
    await sceneRuleCountdown();
    await sceneSettings();
    encodeGif();
  });
});
