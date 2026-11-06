// IBM PC 5150 browser front end: UI thread. The machine runs in
// js/worker.js; this file handles media, controls, keyboard, the screen,
// sound and the debugger panel. Contract: ../spec/spec.md; usability plan:
// ../plans/ux_plan.md.
import { SCAN, isReleaseCombo, CTRL_ALT_DEL } from './keys.js';
import { remember, recall, forget } from './store.js';

const CPU_HZ = 4772727;
const SLOT = { BIOS: 0, BASIC: 1, FONT: 2, A: 3, B: 4 };
const SLOT_NAMES = ['BIOS ROM', 'Cassette BASIC ROM', 'CGA character ROM', 'drive A', 'drive B'];
const EMPTY_TEXT = ['not loaded', 'not loaded — needed for ROM BASIC', 'not loaded — text uses a browser font, no colours', 'empty', 'empty'];
// Recorded originals (../../emulator/original/provenance.md): hash -> [name, slot].
const KNOWN = {
  a259306c47b43e14ef6378d7bdeff67331a2519d363bc36b3f354bc91a6874c8: ['IBM 5150 BIOS, 1981-04-24', SLOT.BIOS],
  '2452b03b4b724b5b81e8cc367809c521144f5fe4e0425caee42ac71240b9b102': ['IBM Cassette BASIC 1.00', SLOT.BASIC],
  '37527f661580e5a09710051cec67422abfadf31c61b841537a91e7f27be19304': ['IBM CGA character ROM', SLOT.FONT],
  '29a4c5eb355b2bfb1dbbece96d91eabc5c1331c6299437d00dd8d862ac126eb2': ['IBM PC DOS 1.00 system diskette', SLOT.A],
  '2404d6897ea480b9b479928151de58d1bdadb7e7d4bc36210766189037e43018': ['the hardware project\'s work diskette (generated)', SLOT.B],
};
const RGB = ['#000000', '#0000aa', '#00aa00', '#00aaaa', '#aa0000', '#aa00aa', '#aa5500', '#aaaaaa',
  '#555555', '#5555ff', '#55ff55', '#55ffff', '#ff5555', '#ff55ff', '#ffff55', '#ffffff'];

const $ = (id) => document.getElementById(id);
const worker = new Worker('js/worker.js');
const screen = $('screen');
const ctx = screen.getContext('2d', { alpha: false });
let ready = false;
let powered = false;
let running = false;
let stoppedByError = false;
const loaded = [false, false, false, false, false];
let audioCtx = null;
let audioNext = 0;
const held = new Set();
// Counters for the browser tests (tests/browser.mjs); not used by the UI.
const stats = { frames: 0, audioChunks: 0, audioSamples: 0, texts: 0 };
window.__pcStats = stats;

function send(msg, transfer) { worker.postMessage(msg, transfer || []); }

function showError(text) {
  const el = $('error');
  el.textContent = text;
  el.hidden = !text;
}

function setStatusCell(slot, text, cls, title) {
  const el = $(`s-${slot}`);
  el.textContent = text;
  el.className = `st ${cls || ''}`;
  el.title = title || '';
  const card = $(`card-${slot}`);
  card.classList.toggle('loaded', cls === 'good');
  card.classList.toggle('user', cls === 'warn');
  card.classList.toggle('bad', cls === 'bad');
}

// ---- Overlay and button state (U2, U3, U4) ----------------------------------
function button(label, onClick, primary) {
  const b = document.createElement('button');
  b.type = 'button';
  b.textContent = label;
  if (primary) b.className = 'primary';
  b.addEventListener('click', onClick);
  return b;
}

function setOverlay(text, actions) {
  const o = $('overlay');
  if (text === null) { o.hidden = true; return; }
  o.hidden = false;
  $('overlay-text').textContent = text;
  $('overlay-actions').replaceChildren(...(actions || []));
}

function refresh() {
  $('power').textContent = powered ? 'Restart' : 'Power on';
  $('power').title = powered ? 'Power cycle: RAM is cleared, disks keep what was written' : '';
  $('power').disabled = !ready || !loaded[SLOT.BIOS];
  $('pause').disabled = !powered;
  $('pause').textContent = running || !powered ? 'Pause' : 'Resume';
  $('cad').disabled = !powered;
  $('type-send').disabled = !powered;
  for (const b of document.querySelectorAll('.keys button')) b.disabled = !powered;
  const chip = $('state');
  const state = stoppedByError ? 'stopped' : !powered ? 'off' : running ? 'running' : 'paused';
  chip.className = `chip ${state}`;
  chip.textContent = { stopped: 'Stopped', off: 'Off', running: 'Running', paused: 'Paused' }[state];

  if (!ready) setOverlay('Loading the emulator…');
  else if (!loaded[SLOT.BIOS]) {
    setOverlay('Load the IBM BIOS ROM to start. Drop your ROM and disk files here, or choose them in “ROMs and disks”.',
      [button('Choose BIOS file…', () => $('f-bios').click(), true)]);
  } else if (!powered) {
    setOverlay(loaded[SLOT.A] ? 'Ready. Drive A has a disk; the BIOS will boot it.'
      : 'Ready. With no disk in drive A the BIOS starts Cassette BASIC (if loaded).',
    [button('Power on', () => powerOn(), true)]);
  } else if (!running) {
    setOverlay(stoppedByError ? 'The machine stopped (see the message below).' : 'Paused.',
      [button(stoppedByError ? 'Restart' : 'Resume', () => (stoppedByError ? powerOn() : send({ type: 'resume' })), true)]);
  } else setOverlay(null);
  updateCapture();
}

function updateCapture() {
  const focused = document.activeElement === screen;
  const cap = $('capture');
  cap.classList.toggle('on', focused && powered);
  cap.textContent = !powered ? 'Keyboard: machine off'
    : focused ? 'Keyboard → PC · Ctrl+] to release' : 'Click the screen to type';
}

// ---- Media (U5, U6) ------------------------------------------------------------
async function sha256Hex(bytes) {
  if (!crypto.subtle) return null; // not a secure context: fall back to size rules
  const d = await crypto.subtle.digest('SHA-256', bytes);
  return [...new Uint8Array(d)].map((b) => b.toString(16).padStart(2, '0')).join('');
}

function loadBytes(slot, bytes, name, from) {
  showError('');
  const keep = $('remember').checked && from !== 'store' && from !== 'generated' ? bytes.slice(0) : null;
  const writeProtect = slot === SLOT.A ? $('wp-a').checked : false;
  pendingStore[slot] = keep ? { name, bytes: keep } : null;
  pendingFrom[slot] = from;
  send({ type: 'load', slot, bytes, writeProtect, name }, [bytes]);
}
const pendingStore = [];
const pendingFrom = [];

async function loadFile(slot, file, from = 'file') {
  setStatusCell(slot, `reading ${file.name}…`);
  loadBytes(slot, await file.arrayBuffer(), file.name, from);
}

for (const input of document.querySelectorAll('input[type=file][data-slot]')) {
  input.addEventListener('change', () => {
    if (input.files && input.files[0]) loadFile(Number(input.dataset.slot), input.files[0]);
    input.value = '';
  });
}

// Where a dropped file goes: a recorded original by its hash, else by size.
async function slotFor(bytes) {
  const known = KNOWN[await sha256Hex(bytes)];
  if (known) return known[1];
  switch (bytes.byteLength) {
    case 8192: return SLOT.BIOS;
    case 2048: return SLOT.FONT;
    case 32768: return SLOT.BASIC;
    case 163840: return loaded[SLOT.A] ? SLOT.B : SLOT.A;
    default: return null;
  }
}

let dragDepth = 0;
window.addEventListener('dragenter', (e) => { if (e.dataTransfer?.types.includes('Files')) { dragDepth++; $('drop').hidden = false; } });
window.addEventListener('dragleave', () => { dragDepth = Math.max(0, dragDepth - 1); if (!dragDepth) $('drop').hidden = true; });
window.addEventListener('dragover', (e) => e.preventDefault());
window.addEventListener('drop', async (e) => {
  e.preventDefault();
  dragDepth = 0;
  $('drop').hidden = true;
  const skipped = [];
  for (const file of e.dataTransfer?.files || []) {
    const bytes = await file.arrayBuffer();
    const slot = await slotFor(bytes);
    if (slot === null) { skipped.push(file.name); continue; }
    if (slot === SLOT.A || slot === SLOT.B) loaded[slot] = true; // so a second disk goes to B
    loadBytes(slot, bytes, file.name, 'file');
  }
  if (skipped.length) showError(`Not recognised (expected a 8 KiB BIOS or character ROM, 32 KiB BASIC, or 160 KiB diskette): ${skipped.join(', ')}`);
});

// A blank 160 KiB data disk PC DOS 1.00 can write to (spec W-A1): both FATs
// start FE FF FF, the 64 directory entries are E5h, data sectors F6h.
function blankDisk() {
  const b = new Uint8Array(163840).fill(0xF6);
  b.fill(0, 0, 7 * 512);
  for (const s of [1, 2]) b.set([0xFE, 0xFF, 0xFF], s * 512);
  for (let e = 0; e < 64; e++) b[3 * 512 + e * 32] = 0xE5;
  return b;
}

$('blank-b').addEventListener('click', () => loadBytes(SLOT.B, blankDisk().buffer, 'blank disk', 'generated'));
$('eject-a').addEventListener('click', () => { send({ type: 'eject', drive: 0 }); forget(SLOT.A); });
$('eject-b').addEventListener('click', () => { send({ type: 'eject', drive: 1 }); forget(SLOT.B); });
$('save-b').addEventListener('click', () => send({ type: 'disk', drive: 1 }));
$('forget').addEventListener('click', async () => {
  for (let s = 0; s < 5; s++) await forget(s);
  $('forget').textContent = 'Forgotten';
  setTimeout(() => { $('forget').textContent = 'Forget'; }, 1500);
});
$('remember').addEventListener('change', async () => {
  if (!$('remember').checked) for (let s = 0; s < 5; s++) await forget(s);
});

async function restoreMedia() {
  const items = await recall();
  for (const { slot, name, bytes } of items) {
    setStatusCell(slot, `restoring ${name}…`);
    loadBytes(slot, bytes, name, 'store');
  }
}

// ---- Controls ------------------------------------------------------------------
function powerOn() {
  showError('');
  stoppedByError = false;
  send({ type: 'power', audio: $('sound').checked });
}
$('power').addEventListener('click', powerOn);
$('pause').addEventListener('click', () => send({ type: running ? 'pause' : 'resume' }));
$('cad').addEventListener('click', () => { send({ type: 'keys', codes: CTRL_ALT_DEL }); screen.focus(); });
$('speed').addEventListener('change', () => send({ type: 'speed', mode: $('speed').value }));
$('sound').addEventListener('change', async () => {
  const on = $('sound').checked;
  if (on && !audioCtx) audioCtx = new AudioContext();
  if (audioCtx) await (on ? audioCtx.resume() : audioCtx.suspend());
  send({ type: 'audio', on, rate: audioCtx ? audioCtx.sampleRate : 44100 });
});

// Special keys (U7): a press and release, or a fixed sequence.
for (const b of document.querySelectorAll('.keys button')) {
  b.addEventListener('click', () => {
    const codes = b.dataset.seq ? b.dataset.seq.split(',').map(Number) : [Number(b.dataset.key), Number(b.dataset.key) | 0x80];
    send({ type: 'keys', codes });
  });
}

$('type-form').addEventListener('submit', (e) => {
  e.preventDefault();
  const text = $('type-text').value + ($('type-enter').checked ? '\r' : '');
  if (text) send({ type: 'type', text });
  $('type-text').value = '';
});

// Backend (U10): switching resets the machine; the SV mode is an explanation.
function setBackend(sv) {
  $('sv-panel').hidden = !sv;
  $('wasm-ui').hidden = sv;
  if (sv && powered) send({ type: 'off' });
  if (sv) {
    ctx.fillStyle = '#000';
    ctx.fillRect(0, 0, screen.width, screen.height);
    $('sv-back').focus();
  }
}
for (const r of document.querySelectorAll('input[name=backend]')) {
  r.addEventListener('change', () => setBackend($('backend-sv').checked));
}
$('sv-back').addEventListener('click', () => {
  $('backend-wasm').checked = true;
  setBackend(false);
  $('backend-wasm').focus();
});

// ---- Keyboard --------------------------------------------------------------------
screen.addEventListener('keydown', (e) => {
  if (isReleaseCombo(e)) {
    e.preventDefault();
    releaseAll();
    $('type-text').focus();
    return;
  }
  const code = SCAN[e.code];
  if (code === undefined || !powered) return;
  e.preventDefault();
  held.add(code);
  send({ type: 'key', code }); // repeats while held: the 5150 keyboard is typematic too
});
screen.addEventListener('keyup', (e) => {
  const code = SCAN[e.code];
  if (code === undefined || !powered) return;
  e.preventDefault();
  held.delete(code);
  send({ type: 'key', code: code | 0x80 });
});
function releaseAll() {
  for (const code of held) send({ type: 'key', code: code | 0x80 });
  held.clear();
}
screen.addEventListener('blur', () => { releaseAll(); updateCapture(); });
screen.addEventListener('focus', updateCapture);
screen.addEventListener('click', () => screen.focus());
$('overlay').addEventListener('click', (e) => { if (e.target === $('overlay') && powered && running) screen.focus(); });

// ---- Screen ------------------------------------------------------------------------
let imageData = null;
function drawPixels(w, h, pixels) {
  if (screen.width !== w || screen.height !== h) {
    screen.width = w;
    screen.height = h;
    imageData = null;
  }
  if (!imageData) imageData = new ImageData(w, h);
  imageData.data.set(pixels);
  ctx.putImageData(imageData, 0, 0);
}

// Without the character ROM: the 25 text rows in a browser font, light grey,
// no attributes (spec W-A2).
function drawText(text, video) {
  const cols = (video & 1) ? 80 : 40;
  const w = cols * 8, h = 200;
  if (screen.width !== w || screen.height !== h) { screen.width = w; screen.height = h; imageData = null; }
  ctx.fillStyle = RGB[0];
  ctx.fillRect(0, 0, w, h);
  if (!(video & 0x08)) return;
  ctx.fillStyle = RGB[7];
  ctx.font = '8px monospace';
  ctx.textBaseline = 'top';
  text.split('\n').forEach((row, r) => {
    for (let c = 0; c < row.length && c < cols; c++) if (row[c] !== ' ') ctx.fillText(row[c], c * 8, r * 8);
  });
}

// Screen as text (U8).
let copyAfterText = false;
$('text-refresh').addEventListener('click', () => send({ type: 'text' }));
$('text-copy').addEventListener('click', () => { copyAfterText = true; send({ type: 'text' }); });
$('textview').addEventListener('toggle', () => { if ($('textview').open) send({ type: 'text' }); });

// ---- Sound ---------------------------------------------------------------------------
function playAudio(samples, rate) {
  if (!audioCtx || audioCtx.state !== 'running') return;
  const buf = audioCtx.createBuffer(1, samples.length, rate);
  buf.copyToChannel(samples, 0);
  const src = audioCtx.createBufferSource();
  src.buffer = buf;
  src.connect(audioCtx.destination);
  const now = audioCtx.currentTime;
  if (audioNext < now + 0.02 || audioNext > now + 0.3) audioNext = now + 0.05;
  src.start(audioNext);
  audioNext += buf.duration;
}

// ---- Debugger (U9) ---------------------------------------------------------------------
function monitor(line) {
  if (!powered) { appendMonitor(`> ${line}\nthe machine is off\n`); return; }
  if (running) send({ type: 'pause' });
  send({ type: 'monitor', line });
}
function appendMonitor(text) {
  const out = $('mon-out');
  out.textContent += text;
  out.scrollTop = out.scrollHeight;
}
for (const b of document.querySelectorAll('button[data-cmd]')) b.addEventListener('click', () => monitor(b.dataset.cmd));
$('mon-clear').addEventListener('click', () => { $('mon-out').textContent = ''; });
$('mon-form').addEventListener('submit', (e) => {
  e.preventDefault();
  const line = $('mon-line').value.trim();
  if (line) monitor(line);
  $('mon-line').select();
});

// ---- Worker messages ---------------------------------------------------------------------
function describe(m) {
  const known = KNOWN[m.sha256];
  const kib = `${(m.size / 1024).toFixed(m.size % 1024 ? 1 : 0)} KiB`;
  const restored = pendingFrom[m.slot] === 'store' ? ' (remembered)' : '';
  return known ? `✓ ${m.name}${restored} · ${kib} · original: ${known[0]}`
    : `${m.name}${restored} · ${kib} · user-provided image (not a recorded original)`;
}

function download(bytes, name) {
  const url = URL.createObjectURL(new Blob([bytes], { type: 'application/octet-stream' }));
  const a = document.createElement('a');
  a.href = url;
  a.download = name;
  document.body.append(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

worker.onmessage = (e) => {
  const m = e.data;
  switch (m.type) {
    case 'ready':
      ready = true;
      $('build').textContent = `Build: ${m.build}.`;
      refresh();
      restoreMedia().catch(() => {});
      break;
    case 'loaded':
      if (m.ok) {
        loaded[m.slot] = true;
        let text = describe(m);
        if (powered && (m.slot === SLOT.BIOS || m.slot === SLOT.BASIC)) text += ' · takes effect at the next restart';
        setStatusCell(m.slot, text, KNOWN[m.sha256] ? 'good' : 'warn', `SHA-256 ${m.sha256}`);
        if (pendingStore[m.slot]) remember(m.slot, pendingStore[m.slot].name, pendingStore[m.slot].bytes).catch(() => {});
      } else {
        loaded[m.slot] = false;
        setStatusCell(m.slot, `not loaded: ${m.error}`, 'bad');
        showError(`${SLOT_NAMES[m.slot]}: ${m.error}`);
        forget(m.slot).catch(() => {});
      }
      pendingStore[m.slot] = null;
      refresh();
      break;
    case 'ejected':
      setStatusCell(SLOT.A + m.drive, EMPTY_TEXT[SLOT.A + m.drive]);
      loaded[SLOT.A + m.drive] = false;
      refresh();
      break;
    case 'powered':
      powered = true;
      running = true;
      refresh();
      screen.focus();
      break;
    case 'frame':
      stats.frames++;
      if (m.width) drawPixels(m.width, m.height, m.pixels);
      else drawText(m.text, m.video);
      send({ type: 'frameAck' });
      break;
    case 'audio':
      stats.audioChunks++;
      stats.audioSamples += m.samples.length;
      playAudio(m.samples, m.rate);
      break;
    case 'status': {
      const changed = powered !== m.powered || running !== m.running;
      powered = m.powered;
      running = m.running;
      if (changed) refresh();
      $('led-a').classList.toggle('on', powered && !!(m.drives & 0x10));
      $('led-b').classList.toggle('on', powered && !!(m.drives & 0x20));
      if (!m.powered) { $('status').textContent = 'Machine off.'; break; }
      const t = (m.clock / CPU_HZ).toFixed(1);
      const pct = m.running && m.speedRatio != null ? ` · ${Math.round(m.speedRatio * 100)}% of a real 5150` : '';
      $('status').textContent = `${t} s of machine time${pct}`;
      break;
    }
    case 'typed':
      if (m.count === 0) showError('Nothing typed: those characters have no key on the US 5150 keyboard.');
      break;
    case 'text':
      stats.texts++;
      $('text-out').textContent = m.text.split('\n').map((r) => r.trimEnd()).join('\n');
      if (copyAfterText) {
        copyAfterText = false;
        navigator.clipboard?.writeText($('text-out').textContent).then(
          () => { $('text-copy').textContent = 'Copied'; setTimeout(() => { $('text-copy').textContent = 'Copy to clipboard'; }, 1500); },
          () => showError('The browser did not allow copying; select the text below instead.'));
      }
      break;
    case 'monitor':
      appendMonitor(`> ${m.line}\n${m.text}`);
      break;
    case 'disk':
      download(m.bytes, m.drive === 1 ? 'drive_b.img' : 'drive_a.img');
      break;
    case 'error':
      if (m.stopped) { stoppedByError = true; running = false; }
      showError(m.message);
      refresh();
      break;
    default:
      break;
  }
};
worker.onerror = (e) => {
  showError(`The emulator could not start: ${e.message || 'the worker failed to load'}. Serve this page over http(s), not file://.`);
  setOverlay('The emulator could not start.');
};

refresh();
send({ type: 'init' });
