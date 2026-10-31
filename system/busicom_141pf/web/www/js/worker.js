// Busicom 141-PF browser machine worker: hosts the WASM emulator core.
// One message = one bounded machine event (spec/spec.md "Execution model");
// the worker never runs loops of its own. All results are read back from
// linear memory after each call because growth invalidates views.
'use strict';

let exports = null;
let decoder = null;

// u32::MAX crosses the C ABI as i32 -1 in JS.
const ERR = -1;

async function loadWasm() {
  // This worker lives in www/js/; the artifact is copied to www/ by
  // scripts/build.sh.
  const res = await fetch('../busicom_emulator_wasm.wasm');
  if (!res.ok) throw new Error(`wasm fetch failed: ${res.status}`);
  const { instance } = await WebAssembly.instantiateStreaming(res, {});
  exports = instance.exports;
  decoder = new TextDecoder();
  return buildId();
}

function outString() {
  const ptr = exports.busicom_out_ptr();
  const len = exports.busicom_out_len();
  if (len === 0) return '';
  return decoder.decode(new Uint8Array(exports.memory.buffer, ptr, len));
}

function buildId() {
  const ptr = exports.busicom_build_id();
  const mem = new Uint8Array(exports.memory.buffer);
  let end = ptr;
  while (mem[end] !== 0) end++;
  return decoder.decode(mem.subarray(ptr, end));
}

function readRows() {
  const text = outString();
  return text.length === 0 ? [] : text.split('\n');
}

// Copy ROM bytes into the module scratch buffer, then build the machine.
function initRom(bytes) {
  const len = bytes.byteLength;
  const scratch = exports.busicom_rom_scratch(len);
  new Uint8Array(exports.memory.buffer, scratch, len).set(new Uint8Array(bytes));
  return exports.busicom_init(len);
}

self.onmessage = async (e) => {
  const msg = e.data;
  try {
    switch (msg.type) {
      case 'load': {
        const build = await loadWasm();
        const rc = initRom(msg.rom);
        post({
          type: 'loaded', ok: rc !== ERR, build,
          error: rc === ERR ? outString() : null,
        });
        break;
      }
      case 'key': {
        // One key/switch event: selectors, matrix code, 400 half-spins
        // (the deterministic-schedule press length).
        const t0 = performance.now();
        const rc = exports.busicom_advance(msg.precision, msg.rounding, msg.key, msg.halfSpins);
        if (rc === ERR) throw new Error(outString());
        post({
          type: 'rows', rows: readRows(), lamps: exports.busicom_lamps(),
          ms: performance.now() - t0,
        });
        break;
      }
      case 'reset': {
        const rc = exports.busicom_reset();
        if (rc === ERR) throw new Error(outString());
        post({ type: 'reset', lamps: exports.busicom_lamps() });
        break;
      }
      case 'lamps': {
        post({ type: 'lamps', lamps: exports.busicom_lamps() });
        break;
      }
      default:
        throw new Error(`unknown message type ${msg.type}`);
    }
  } catch (err) {
    post({ type: 'error', message: String(err && err.message || err) });
  }
};

function post(msg) {
  self.postMessage(msg);
}
