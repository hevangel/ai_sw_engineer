// Node verification for the Busicom 141-PF browser machine WASM binding
// (plans/testplan.md T1 + T2). Run from the repository root:
//
//   node system/busicom_141pf/web/tests/node_replay.mjs
//   BUSICOM_ROM=/path/to/rom_141pf_combined.bin \
//   node system/busicom_141pf/web/tests/node_replay.mjs \
//        --schedule <schedule.txt> --trace <rom_port_trace.txt>
//
// The default self-test needs no ROM: it drives the same committed 8-byte
// lamp program as the native emulator unit tests and checks the error paths.
// The replay test loads the user-provided original ROM, replays the given
// schedule through the WASM exports and requires the payload to equal the
// committed original-ROM trace byte for byte. Emission order mirrors the
// native Emulator::replay: every #STEP marker is followed by LAMPS|n.
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const web = join(dirname(fileURLToPath(import.meta.url)), '..');
const wasmPath = join(web, 'www', 'busicom_emulator_wasm.wasm');

// u32::MAX crosses the C ABI as i32 -1 in JS.
const ERR = -1;

function parseArgs(argv) {
  const args = { _: [] };
  for (let i = 0; i < argv.length; i++) {
    if (argv[i].startsWith('--')) args[argv[i].slice(2)] = argv[++i] ?? true;
    else args._.push(argv[i]);
  }
  return args;
}

function cString(memory, ptr) {
  const mem = new Uint8Array(memory.buffer);
  let end = ptr;
  while (mem[end] !== 0) end++;
  return new TextDecoder().decode(mem.subarray(ptr, end));
}

function outString(exports) {
  const len = exports.busicom_out_len();
  if (len === 0) return '';
  const ptr = exports.busicom_out_ptr();
  return new TextDecoder().decode(new Uint8Array(exports.memory.buffer, ptr, len));
}

async function instantiate() {
  const bytes = readFileSync(wasmPath);
  const { instance } = await WebAssembly.instantiate(bytes, {});
  return instance.exports;
}

// The committed lamp stimulus from emulator/tests/replay.rs: FIM P0,0x40;
// SRC P0; LDM 5; WMP; JUN 0 — P0 selects RAM1, WMP writes the lamp nibble.
const LAMP_PROGRAM = [0x20, 0x40, 0x21, 0xd5, 0xe1, 0x00, 0x40, 0x06];

async function selftest() {
  const exports = await instantiate();
  const expect = (cond, msg) => {
    if (!cond) {
      console.error(`FAIL: ${msg}`);
      process.exitCode = 1;
    } else {
      console.log(`ok: ${msg}`);
    }
  };

  for (const name of ['busicom_rom_scratch', 'busicom_init', 'busicom_reset',
    'busicom_lamps', 'busicom_advance', 'busicom_out_ptr', 'busicom_out_len',
    'busicom_build_id', 'memory']) {
    expect(exports[name] !== undefined, `export ${name} exists`);
  }
  expect(cString(exports.memory, exports.busicom_build_id()).startsWith('busicom-emulator-wasm'),
    'build id identifies the binding');

  // Empty image must fail with the message in the output buffer.
  exports.busicom_rom_scratch(0);
  expect(exports.busicom_init(0) === ERR && outString(exports).length > 0,
    'init with an empty image fails with a message');

  // The lamp program drives lamps to 5 and reset clears them.
  const scratch = exports.busicom_rom_scratch(LAMP_PROGRAM.length);
  new Uint8Array(exports.memory.buffer, scratch, LAMP_PROGRAM.length)
    .set(Uint8Array.from(LAMP_PROGRAM));
  expect(exports.busicom_init(LAMP_PROGRAM.length) === 0, 'lamp program loads');

  const settle = exports.busicom_advance(2, 0, 0, 1);
  expect(settle === 0, 'settle event prints nothing');
  expect(exports.busicom_lamps() === 5, `lamps read 5 (got ${exports.busicom_lamps()})`);
  expect(exports.busicom_reset() === 0 && exports.busicom_lamps() === 0,
    'reset clears the lamps');

  // Invalid inputs are rejected exactly like the native API.
  exports.busicom_rom_scratch(1280);
  expect(exports.busicom_init(1280) === 0, 'a blank padded image loads');
  expect(exports.busicom_advance(7, 0, 0, 1) === ERR, 'precision 7 is rejected');
  expect(exports.busicom_advance(2, 2, 0, 1) === ERR, 'rounding 2 is rejected');
  // The native API rejects codes outside 0 | 129..=160 (same as the native
  // unit test's "2 0 128 1"); the unpopulated 143/144 positions are inside
  // that range and are the native model's business, not the binding's.
  expect(exports.busicom_advance(2, 0, 128, 1) === ERR, 'key 128 is rejected');
  expect(exports.busicom_advance(2, 0, 161, 1) === ERR, 'key 161 is rejected');

  if (process.exitCode) throw new Error('self-test failed');
  console.log('PASS: WASM binding self-test');
}

async function replay(schedulePath, tracePath) {
  const romPath = process.env.BUSICOM_ROM;
  if (!romPath) throw new Error('set BUSICOM_ROM to the provenance-hashed original ROM');
  const rom = readFileSync(romPath);
  const schedule = readFileSync(schedulePath, 'utf8');
  const want = readFileSync(tracePath, 'utf8').split('\n')
    .filter((l) => l !== '' && !l.startsWith('#'))
    .join('\n');

  const exports = await instantiate();
  const scratch = exports.busicom_rom_scratch(rom.length);
  new Uint8Array(exports.memory.buffer, scratch, rom.length).set(rom);
  if (exports.busicom_init(rom.length) === ERR) {
    throw new Error(`init failed: ${outString(exports)}`);
  }

  // Markers drive reset/lamps timing but are not payload: the committed
  // trace payload is ROW|… and LAMPS|… lines only.
  const payload = [];
  for (const line of schedule.split('\n')) {
    const l = line.trim();
    if (l === '' || l.startsWith('# ')) continue;
    const f = l.split(/\s+/);
    if (f[0] === '#EXAMPLE') {
      exports.busicom_reset();
    } else if (f[0] === '#RESET') {
      // checkpoint only
    } else if (f[0] === '#STEP') {
      payload.push(`LAMPS|${exports.busicom_lamps()}`);
    } else {
      const rc = exports.busicom_advance(Number(f[0]), Number(f[1]), Number(f[2]), Number(f[3]));
      if (rc === ERR) throw new Error(`advance rejected ${l}: ${outString(exports)}`);
      const rows = outString(exports);
      if (rows.length > 0) payload.push(...rows.split('\n'));
    }
  }

  const got = payload.join('\n');
  if (got !== want) {
    const g = got.split('\n');
    const w = want.split('\n');
    for (let i = 0; i < Math.max(g.length, w.length); i++) {
      if (g[i] !== w[i]) {
        console.error(`first difference at payload line ${i + 1}:\n  got:  ${g[i]}\n  want: ${w[i]}`);
        break;
      }
    }
    throw new Error(`payload mismatch: ${g.length} lines vs ${w.length}`);
  }
  console.log(`PASS: WASM replay matches the committed original-ROM trace (${payload.length} payload lines)`);
}

const args = parseArgs(process.argv.slice(2));
if (args.schedule && args.trace) {
  await replay(args.schedule, args.trace);
} else {
  await selftest();
}
