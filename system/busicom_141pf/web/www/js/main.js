// Busicom 141-PF browser machine UI. All execution happens in worker.js;
// this module only builds the controls, forwards bounded events and renders
// rows/lamps (spec/spec.md "UI contract").
'use strict';

import { KEYPAD_ROWS, KEY_MAP } from './keys.js';

// Recorded provenance hash of the original firmware
// (../../firmware/original/provenance.md). The page never bundles the image;
// it only recognizes a user-provided copy by this hash.
const ORIGINAL_SHA256 = 'c6af325d64b0895a8c6f7298e5443bbf5814783fabd6a1bcd2df08a9bec2ceb7';

const OVERFLOW_VALUE = '.'.repeat(15); // the dotted overflow line

const paper = document.getElementById('paper');
const status = document.getElementById('status');
const power = document.getElementById('power');
const romFile = document.getElementById('rom-file');
const romInfo = document.getElementById('rom-info');
const backend = document.getElementById('backend');
const svNotice = document.getElementById('sv-unavailable');
const dpSel = document.getElementById('dp');
const roundSel = document.getElementById('rounding');

const worker = new Worker(new URL('./worker.js', import.meta.url));
let loaded = false;
let busy = false;

function selectors() {
  return {
    precision: Number(dpSel.value),
    rounding: Number(roundSel.value),
  };
}

worker.onmessage = (e) => {
  const msg = e.data;
  busy = false;
  switch (msg.type) {
    case 'loaded':
      if (msg.ok) {
        loaded = true;
        power.disabled = false;
        document.getElementById('build').textContent = msg.build;
        statusMessage(`running — ${msg.build}`);
        powerOn();
      } else {
        statusMessage(`load failed: ${msg.error}`);
      }
      break;
    case 'rows':
      for (const line of msg.rows) appendRow(line);
      setLamps(msg.lamps);
      break;
    case 'reset':
      clearPaper();
      setLamps(msg.lamps);
      break;
    case 'error':
      statusMessage(`machine error: ${msg.message}`);
      break;
    default:
  }
};

worker.onerror = (e) => statusMessage(`worker error: ${e.message}`);

function statusMessage(text) {
  status.textContent = text;
}

function powerOn() {
  // Same settle event the deterministic schedule uses (2 0 0 2000): two
  // drum revolutions with no key held, at the current selector positions.
  const sel = selectors();
  worker.postMessage({ type: 'reset' });
  worker.postMessage({ type: 'key', key: 0, halfSpins: 2000, ...sel });
}

function press(code) {
  if (!loaded || busy) return;
  busy = true;
  worker.postMessage({ type: 'key', key: code, halfSpins: 400, ...selectors() });
}

function appendRow(line) {
  // ROW|value|symA|symB|red — the oracle tape format.
  const parts = line.split('|');
  if (parts.length !== 5 || parts[0] !== 'ROW') return;
  const [, value, symA, symB, red] = parts;
  const row = document.createElement('div');
  row.className = 'row' + (red === 'true' ? ' red' : '');
  const overflow = value === OVERFLOW_VALUE;
  if (overflow) row.classList.add('overflow');
  const val = document.createElement('span');
  val.className = 'value';
  val.textContent = overflow ? '···············' : value;
  const sym = document.createElement('span');
  sym.className = 'sym';
  sym.textContent = (symA + ' ' + symB).trim();
  row.append(val, sym);
  paper.appendChild(row);
  paper.scrollTop = paper.scrollHeight;
}

function setLamps(bits) {
  document.getElementById('lamp-memory').classList.toggle('on', (bits & 1) !== 0);
  document.getElementById('lamp-overflow').classList.toggle('on', (bits & 2) !== 0);
  document.getElementById('lamp-negative').classList.toggle('on', (bits & 4) !== 0);
}

function clearPaper() {
  paper.replaceChildren();
}

async function loadRomFile(file) {
  const bytes = await file.arrayBuffer();
  const size = bytes.byteLength;
  if (size < 1 || size > 4096) {
    statusMessage(`image must be 1..=4096 bytes, got ${size}`);
    return;
  }
  const hash = await crypto.subtle.digest('SHA-256', bytes);
  const hex = [...new Uint8Array(hash)].map((b) => b.toString(16).padStart(2, '0')).join('');
  romInfo.hidden = false;
  document.getElementById('rom-size').textContent =
    `${file.name}: ${size} bytes`;
  document.getElementById('rom-hash').textContent = `SHA-256 ${hex}`;
  const prov = document.getElementById('rom-provenance');
  if (hex === ORIGINAL_SHA256) {
    prov.textContent = 'original Busicom firmware (matches recorded provenance hash)';
    prov.className = 'match';
  } else {
    prov.textContent = 'user-provided image (hash does not match the recorded original)';
    prov.className = 'mismatch';
  }
  statusMessage('loading…');
  loaded = false;
  power.disabled = true;
  clearPaper();
  setLamps(0);
  worker.postMessage({ type: 'load', rom: bytes }, [bytes]);
}

power.addEventListener('click', () => loaded && powerOn());
romFile.addEventListener('change', () => romFile.files[0] && loadRomFile(romFile.files[0]));

// Build the keypad in the physical layout; data-key records the PC mapping.
const keypad = document.getElementById('keypad');
for (const row of KEYPAD_ROWS) {
  for (let col = 0; col < 4; col++) {
    const k = row[col];
    if (!k) {
      const gap = document.createElement('span');
      gap.className = 'gap';
      keypad.appendChild(gap);
      continue;
    }
    const btn = document.createElement('button');
    btn.textContent = k.label;
    btn.dataset.code = k.code;
    if (k.title) btn.title = k.title;
    if (k.code >= 133 && k.code <= 142) btn.classList.add('op');
    btn.addEventListener('click', () => press(k.code));
    keypad.appendChild(btn);
  }
}

document.addEventListener('keydown', (e) => {
  if (e.ctrlKey || e.metaKey || e.altKey) return;
  const code = KEY_MAP.get(e.key.toLowerCase());
  if (code === undefined) return;
  e.preventDefault();
  press(code);
});

backend.addEventListener('change', () => {
  const sv = backend.value === 'sv';
  svNotice.hidden = !sv;
  if (sv) statusMessage('SystemVerilog backend unavailable — default Rust/WASM machine stays active');
  else statusMessage('default Rust/WASM backend selected');
});
