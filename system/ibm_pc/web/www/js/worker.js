// IBM PC 5150 browser machine worker: hosts the WASM build of the native
// Rust emulator and runs it in bounded batches (../spec/spec.md, "Execution
// model"). Messages from the page are handled between batches, so input
// stays responsive. Views of linear memory are re-made after every call,
// because the module's memory can grow.
'use strict';

const CPU_HZ = 4772727;           // 14.31818 MHz / 3
const ERR = 0xFFFFFFFF;
const MAX_BATCH_MS = 50;          // never catch up more than this per tick
const TICK_MS = 4;                // pause between batches (yields to messages)
const MAX_SPEED_BUDGET_MS = 12;   // wall time spent per tick in "max" speed
const STATUS_MS = 250;
const FRAME_MS = 1000 / 60;        // the CGA's refresh rate; no point drawing faster

let x = null;                     // wasm exports
let running = false;              // the run loop is active
let powered = false;
let speed = 'real';               // 'real' | 'max'
let audioOn = false;
let audioRate = 44100;
let frameWanted = true;           // the page acknowledged the last frame
let last = 0;
let carry = 0;
let lastStatus = 0;
let lastFrame = 0;
let wallStart = 0, clockStart = 0;
let timer = null;
// DC blocker state for the speaker (its idle level is a constant).
let dcX = 0, dcY = 0;

const u32 = (v) => v >>> 0;
const enc = new TextEncoder();
const dec = new TextDecoder();

function out() {
  return dec.decode(new Uint8Array(x.memory.buffer, x.pc_out_ptr(), x.pc_out_len()));
}

function put(bytes) {
  const p = x.pc_scratch(bytes.length);
  new Uint8Array(x.memory.buffer, p, bytes.length).set(bytes);
  return bytes.length;
}

function buildId() {
  const mem = new Uint8Array(x.memory.buffer);
  let p = x.pc_build_id(), e = p;
  while (mem[e] !== 0) e++;
  return dec.decode(mem.subarray(p, e));
}

async function init() {
  const res = await fetch('../ibm_pc_emulator_wasm.wasm');
  if (!res.ok) throw new Error(`could not fetch the emulator (ibm_pc_emulator_wasm.wasm): HTTP ${res.status}`);
  const { instance } = await WebAssembly.instantiateStreaming(res, {});
  x = instance.exports;
  return buildId();
}

function status(extra = {}) {
  const clock = x.pc_clock();
  const now = performance.now();
  const wall = (now - wallStart) / 1000;
  const emu = (clock - clockStart) / CPU_HZ;
  const ratio = wall > 0.2 ? emu / wall : null;
  if (wall > 2) { wallStart = now; clockStart = clock; }
  post({
    type: 'status', running, powered, clock, instructions: x.pc_instructions(),
    speedRatio: ratio, drives: x.pc_drive_status(), keysPending: x.pc_keys_pending(),
    ...extra,
  });
}

function sendFrame() {
  frameWanted = false;
  const wh = u32(x.pc_frame());
  if (wh === 0) {
    x.pc_text();
    post({ type: 'frame', width: 0, height: 0, text: out(), video: u32(x.pc_video_state()) });
    return;
  }
  const w = wh >>> 16, h = wh & 0xFFFF;
  const pixels = new Uint8ClampedArray(w * h * 4);
  pixels.set(new Uint8ClampedArray(x.memory.buffer, x.pc_frame_ptr(), w * h * 4));
  post({ type: 'frame', width: w, height: h, pixels }, [pixels.buffer]);
}

function sendAudio() {
  const n = u32(x.pc_audio(audioRate));
  if (!audioOn || n === 0 || speed !== 'real') return;
  const src = new Float32Array(x.memory.buffer, x.pc_audio_ptr(), n);
  const samples = new Float32Array(n);
  for (let i = 0; i < n; i++) {
    const y = src[i] - dcX + 0.995 * dcY;
    dcX = src[i];
    dcY = y;
    samples[i] = 0.25 * y;
  }
  post({ type: 'audio', samples, rate: audioRate }, [samples.buffer]);
}

function stopWith(message) {
  running = false;
  post({ type: 'error', message });
  status();
}

function tick() {
  timer = null;
  if (!running) return;
  const now = performance.now();
  const dt = Math.min(now - last, MAX_BATCH_MS);
  last = now;
  if (speed === 'max') {
    const until = now + MAX_SPEED_BUDGET_MS;
    do {
      const rc = u32(x.pc_run(Math.round(CPU_HZ / 200)));
      if (rc === ERR) return stopWith(out());
    } while (performance.now() < until);
  } else {
    const want = dt * CPU_HZ / 1000 + carry;
    const clocks = Math.floor(want);
    carry = want - clocks;
    if (clocks > 0) {
      const rc = u32(x.pc_run(clocks));
      if (rc === ERR) return stopWith(out());
    }
  }
  sendAudio();
  if (frameWanted && now - lastFrame >= FRAME_MS - 1) { lastFrame = now; sendFrame(); }
  if (now - lastStatus > STATUS_MS) { lastStatus = now; status(); }
  timer = setTimeout(tick, TICK_MS);
}

function start() {
  if (running || !powered) return;
  running = true;
  last = performance.now();
  wallStart = last;
  clockStart = x.pc_clock();
  carry = 0;
  timer = setTimeout(tick, 0);
}

function pause() {
  running = false;
  if (timer !== null) { clearTimeout(timer); timer = null; }
}

const handlers = {
  async init() {
    const build = await init();
    post({ type: 'ready', build });
  },
  load({ slot, bytes, writeProtect, name }) {
    const data = new Uint8Array(bytes);
    x.pc_sha256(put(data));
    const sha256 = out();
    const rc = u32(x.pc_load(slot, put(data), writeProtect ? 1 : 0));
    post({ type: 'loaded', slot, name, size: data.length, sha256, ok: rc !== ERR, error: rc === ERR ? out() : null });
  },
  eject({ drive }) {
    x.pc_eject(drive);
    post({ type: 'ejected', drive });
  },
  power({ audio }) {
    pause();
    const rc = u32(x.pc_power_on(audio ? 1 : 0));
    if (rc === ERR) {
      powered = false;
      post({ type: 'error', message: out() });
      status();
      return;
    }
    powered = true;
    dcX = dcY = 0;
    frameWanted = true;
    post({ type: 'powered' });
    start();
    status();
  },
  off() {
    pause();
    powered = false;
    status();
  },
  pause() { pause(); if (powered) { frameWanted = true; sendFrame(); } status(); },
  resume() { start(); status(); },
  speed({ mode }) { speed = mode === 'max' ? 'max' : 'real'; status(); },
  audio({ on, rate }) {
    audioOn = !!on;
    if (rate) audioRate = rate;
    x.pc_set_audio(audioOn ? 1 : 0);
  },
  key({ code }) { x.pc_key(code); },
  keys({ codes }) { for (const c of codes) x.pc_key(c); },
  type({ text }) {
    const n = u32(x.pc_type(put(enc.encode(text.replace(/\n/g, '\r')))));
    post({ type: 'typed', count: n });
  },
  frameAck() { frameWanted = true; if (!running && powered) sendFrame(); },
  monitor({ line }) {
    const rc = u32(x.pc_monitor(put(enc.encode(line))));
    post({ type: 'monitor', line, text: out(), error: rc === ERR });
    if (powered) { frameWanted = true; sendFrame(); }
    status();
  },
  disk({ drive }) {
    const rc = u32(x.pc_disk(drive));
    if (rc === ERR) { post({ type: 'error', message: out() }); return; }
    const bytes = new Uint8Array(x.memory.buffer, x.pc_out_ptr(), x.pc_out_len()).slice();
    post({ type: 'disk', drive, bytes }, [bytes.buffer]);
  },
  status() { status(); },
};

self.onmessage = async (e) => {
  const msg = e.data;
  try {
    const h = handlers[msg.type];
    if (!h) throw new Error(`unknown message ${msg.type}`);
    await h(msg);
  } catch (err) {
    post({ type: 'error', message: String((err && err.message) || err) });
  }
};

function post(msg, transfer) {
  self.postMessage(msg, transfer || []);
}
