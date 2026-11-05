// IBM PC 5150 browser front end: UI thread. The machine runs in
// js/worker.js; this file handles media, controls, keyboard, the screen,
// sound and the debugger panel. Contract: ../spec/spec.md.
import { SCAN, isReleaseCombo, CTRL_ALT_DEL } from './keys.js';

const CPU_HZ = 4772727;
const SLOT = { BIOS: 0, BASIC: 1, FONT: 2, A: 3, B: 4 };
const SLOT_NAMES = ['BIOS ROM', 'Cassette BASIC ROM', 'CGA character ROM', 'drive A', 'drive B'];
// Recorded originals (../../emulator/original/provenance.md).
const KNOWN = {
  a259306c47b43e14ef6378d7bdeff67331a2519d363bc36b3f354bc91a6874c8: 'IBM 5150 BIOS, 1981-04-24',
  '2452b03b4b724b5b81e8cc367809c521144f5fe4e0425caee42ac71240b9b102': 'IBM Cassette BASIC 1.00',
  '37527f661580e5a09710051cec67422abfadf31c61b841537a91e7f27be19304': 'IBM CGA character ROM',
  '29a4c5eb355b2bfb1dbbece96d91eabc5c1331c6299437d00dd8d862ac126eb2': 'IBM PC DOS 1.00 system diskette',
  '2404d6897ea480b9b479928151de58d1bdadb7e7d4bc36210766189037e43018': 'the hardware project\'s work diskette (generated)',
};
// RGB of the 16 CGA colours (for the text fallback without a character ROM).
const RGB = ['#000000', '#0000aa', '#00aa00', '#00aaaa', '#aa0000', '#aa00aa', '#aa5500', '#aaaaaa',
  '#555555', '#5555ff', '#55ff55', '#55ffff', '#ff5555', '#ff55ff', '#ffff55', '#ffffff'];

const $ = (id) => document.getElementById(id);
const worker = new Worker('js/worker.js');
const screen = $('screen');
const ctx = screen.getContext('2d', { alpha: false });
let ready = false;
let powered = false;
let running = false;
let loaded = [false, false, false, false, false];
let audioCtx = null;
let audioNext = 0;
// Counters for the browser tests (tests/browser.mjs); not used by the UI.
const stats = { frames: 0, audioChunks: 0, audioSamples: 0 };
window.__pcStats = stats;
const held = new Set();

function send(msg, transfer) { worker.postMessage(msg, transfer || []); }

function showError(text) {
  const el = $('error');
  el.textContent = text;
  el.hidden = !text;
}

function setStatusCell(slot, text, cls) {
  const el = $(`s-${slot}`);
  el.textContent = text;
  el.className = `st ${cls || ''}`;
}

function updateButtons() {
  $('power').textContent = powered ? 'Restart (power cycle)' : 'Power on';
  $('power').disabled = !ready;
  $('pause').disabled = !powered;
  $('pause').textContent = running ? 'Pause' : 'Resume';
  $('cad').disabled = !powered;
  $('type-send').disabled = !powered;
}

// ---- Media ---------------------------------------------------------------
async function loadFile(slot, file) {
  showError('');
  setStatusCell(slot, `reading ${file.name}…`);
  const bytes = await file.arrayBuffer();
  const writeProtect = slot === SLOT.A ? $('wp-a').checked : false;
  send({ type: 'load', slot, bytes, writeProtect, name: file.name }, [bytes]);
}

for (const input of document.querySelectorAll('input[type=file][data-slot]')) {
  input.addEventListener('change', () => {
    if (input.files && input.files[0]) loadFile(Number(input.dataset.slot), input.files[0]);
  });
}

// A blank 160 KiB data disk PC DOS 1.00 can write to: both FATs start
// FE FF FF (single-sided, 8 sectors), the 64 directory entries are E5h
// (unused; DOS 1.00 counts entries starting 00h as files), data sectors F6h.
// Not byte-identical to what FORMAT writes (its boot record is not copied).
function blankDisk() {
  const b = new Uint8Array(163840).fill(0xF6);
  b.fill(0, 0, 7 * 512);
  for (const s of [1, 2]) b.set([0xFE, 0xFF, 0xFF], s * 512);
  for (let e = 0; e < 64; e++) b[3 * 512 + e * 32] = 0xE5;
  return b;
}

$('blank-b').addEventListener('click', () => {
  const bytes = blankDisk().buffer;
  send({ type: 'load', slot: SLOT.B, bytes, writeProtect: false, name: 'blank disk' }, [bytes]);
});
$('eject-a').addEventListener('click', () => send({ type: 'eject', drive: 0 }));
$('eject-b').addEventListener('click', () => send({ type: 'eject', drive: 1 }));
$('save-b').addEventListener('click', () => send({ type: 'disk', drive: 1 }));

// ---- Controls --------------------------------------------------------------
$('power').addEventListener('click', () => {
  showError('');
  send({ type: 'power', audio: $('sound').checked });
});
$('pause').addEventListener('click', () => send({ type: running ? 'pause' : 'resume' }));
$('cad').addEventListener('click', () => { send({ type: 'keys', codes: CTRL_ALT_DEL }); screen.focus(); });
$('speed').addEventListener('change', () => send({ type: 'speed', mode: $('speed').value }));
$('sound').addEventListener('change', async () => {
  const on = $('sound').checked;
  if (on && !audioCtx) {
    audioCtx = new AudioContext();
  }
  if (audioCtx) await (on ? audioCtx.resume() : audioCtx.suspend());
  send({ type: 'audio', on, rate: audioCtx ? audioCtx.sampleRate : 44100 });
});

$('type-form').addEventListener('submit', (e) => {
  e.preventDefault();
  const text = $('type-text').value + ($('type-enter').checked ? '\r' : '');
  if (text) send({ type: 'type', text });
  $('type-text').value = '';
});

$('backend').addEventListener('change', () => {
  const sv = $('backend').value === 'sv';
  $('sv-panel').hidden = !sv;
  $('wasm-ui').hidden = sv;
  // Switching backends resets the machine (no cross-backend state transfer).
  if (sv && powered) send({ type: 'off' });
  if (sv) $('sv-back').focus();
});
$('sv-back').addEventListener('click', () => {
  $('backend').value = 'wasm';
  $('backend').dispatchEvent(new Event('change'));
  $('backend').focus();
});

// ---- Keyboard --------------------------------------------------------------
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
screen.addEventListener('blur', releaseAll);
screen.addEventListener('click', () => screen.focus());

// ---- Screen ----------------------------------------------------------------
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

// Without the character ROM: draw the 25 text rows with a browser font.
// The colour attributes are not available on this path; it is light grey on
// black, labelled as an approximation in the media table.
function drawText(text, video) {
  const cols = (video & 1) ? 80 : 40;
  const w = cols * 8, h = 200;
  if (screen.width !== w || screen.height !== h) { screen.width = w; screen.height = h; imageData = null; }
  ctx.fillStyle = RGB[0];
  ctx.fillRect(0, 0, w, h);
  if (!(video & 0x08)) return; // display disabled
  ctx.fillStyle = RGB[7];
  ctx.font = '8px monospace';
  ctx.textBaseline = 'top';
  const rows = text.split('\n');
  rows.forEach((row, r) => {
    for (let c = 0; c < row.length && c < cols; c++) {
      if (row[c] !== ' ') ctx.fillText(row[c], c * 8, r * 8);
    }
  });
}

// ---- Sound -----------------------------------------------------------------
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

// ---- Debugger --------------------------------------------------------------
function monitor(line) {
  if (!powered) { $('mon-out').textContent += 'the machine is off\n'; return; }
  send({ type: 'monitor', line });
}
for (const b of document.querySelectorAll('button[data-cmd]')) {
  b.addEventListener('click', () => monitor(b.dataset.cmd));
}
$('mon-form').addEventListener('submit', (e) => {
  e.preventDefault();
  const line = $('mon-line').value.trim();
  if (line) monitor(line);
  $('mon-line').select();
});

// ---- Worker messages ---------------------------------------------------------
function describe(msg) {
  const known = KNOWN[msg.sha256];
  const kib = `${(msg.size / 1024).toFixed(msg.size % 1024 ? 1 : 0)} KiB`;
  const who = known ? `original: ${known}` : 'user-provided image (not a recorded original)';
  return `${msg.name}, ${kib}, ${who}. SHA-256 ${msg.sha256.slice(0, 16)}…`;
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
      updateButtons();
      break;
    case 'loaded':
      if (m.ok) {
        loaded[m.slot] = true;
        setStatusCell(m.slot, describe(m), KNOWN[m.sha256] ? 'good' : 'warn');
        if (powered && (m.slot === SLOT.BIOS || m.slot === SLOT.BASIC)) {
          setStatusCell(m.slot, `${describe(m)} Takes effect at the next restart.`, 'warn');
        }
      } else {
        setStatusCell(m.slot, `not loaded: ${m.error}`, 'bad');
        showError(`${SLOT_NAMES[m.slot]}: ${m.error}`);
      }
      break;
    case 'ejected':
      setStatusCell(SLOT.A + m.drive, 'empty');
      loaded[SLOT.A + m.drive] = false;
      break;
    case 'powered':
      powered = true;
      running = true;
      updateButtons();
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
      powered = m.powered;
      running = m.running;
      updateButtons();
      if (!m.powered) { $('status').textContent = 'Machine off.'; break; }
      const t = (m.clock / CPU_HZ).toFixed(1);
      const pct = m.speedRatio == null ? '' : ` at ${Math.round(m.speedRatio * 100)}% of a real 5150`;
      const motors = ['A', 'B'].filter((_, i) => m.drives & (0x10 << i));
      const drive = motors.length ? ` Drive ${motors.join(', ')} motor on.` : '';
      $('status').textContent = `${m.running ? 'Running' : 'Paused'}: ${t} s of machine time${m.running ? pct : ''}.${drive}`;
      break;
    }
    case 'typed':
      if (m.count === 0) showError('Nothing typed: those characters have no key on the US 5150 keyboard.');
      break;
    case 'monitor': {
      const out = $('mon-out');
      out.textContent += `> ${m.line}\n${m.text}`;
      out.scrollTop = out.scrollHeight;
      break;
    }
    case 'disk':
      download(m.bytes, m.drive === 1 ? 'drive_b.img' : 'drive_a.img');
      break;
    case 'error':
      showError(m.message);
      break;
    default:
      break;
  }
};
worker.onerror = (e) => showError(`The emulator worker failed: ${e.message}`);

updateButtons();
send({ type: 'init' });
