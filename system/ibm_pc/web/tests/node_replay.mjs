// Node checks of the browser machine's WASM build (no browser needed).
//
//   node system/ibm_pc/web/tests/node_replay.mjs            # self-test, no ROMs
//   IBM_PC_ROMS=DIR node system/ibm_pc/web/tests/node_replay.mjs
//
// With IBM_PC_ROMS (bios.bin, basic.bin, pcdos100.img, workdisk.img; see
// ../../emulator/original/provenance.md) it replays the hardware project's RTL
// transcript (../../emulator/oracle/transcripts/dos100_session.txt) through
// the WASM exports and requires every screen, video-memory hash and drive B
// hash to be identical: the expected output is the RTL's, not this build's.
// It also measures how fast the WASM machine runs relative to the 5150.
import { readFileSync, existsSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const wasmPath = join(here, '../www/ibm_pc_emulator_wasm.wasm');
const transcriptPath = join(here, '../../emulator/oracle/transcripts/dos100_session.txt');
const ERR = 0xFFFFFFFF;
const CPU_HZ = 4772727;
const SHA = {
  'bios.bin': 'a259306c47b43e14ef6378d7bdeff67331a2519d363bc36b3f354bc91a6874c8',
  'basic.bin': '2452b03b4b724b5b81e8cc367809c521144f5fe4e0425caee42ac71240b9b102',
  'pcdos100.img': '29a4c5eb355b2bfb1dbbece96d91eabc5c1331c6299437d00dd8d862ac126eb2',
};

let failures = 0;
const check = (ok, what) => {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${what}`);
  if (!ok) failures++;
};

const { instance } = await WebAssembly.instantiate(readFileSync(wasmPath), {});
const x = instance.exports;
const u32 = (v) => v >>> 0;
const out = () => new TextDecoder().decode(new Uint8Array(x.memory.buffer, x.pc_out_ptr(), x.pc_out_len()));
const put = (bytes) => {
  const p = x.pc_scratch(bytes.length);
  new Uint8Array(x.memory.buffer, p, bytes.length).set(bytes);
  return bytes.length;
};
const load = (slot, bytes, wp = 0) => u32(x.pc_load(slot, put(bytes), wp));

// --- Self-test: no copyrighted input. A stand-in ROM (not IBM's) writes 'A'
// to the CGA text buffer and halts.
{
  const rom = new Uint8Array(8192).fill(0x90);
  rom.set([0xB8, 0x00, 0xB8, 0x8E, 0xC0, 0x26, 0xC7, 0x06, 0x00, 0x00, 0x41, 0x07,
    0xB0, 0x09, 0xBA, 0xD8, 0x03, 0xEE, 0xF4], 0);
  rom.set([0xEA, 0x00, 0xE0, 0x00, 0xF0], 0x1FF0);
  check(u32(x.pc_power_on(0)) === ERR && /BIOS/.test(out()), 'power on without a BIOS is refused with a reason');
  check(load(0, new Uint8Array(100)) === ERR && /8,192/.test(out()), 'a wrong-size BIOS is refused with a reason');
  check(load(3, new Uint8Array(1000)) === ERR && /163,840/.test(out()), 'a wrong-size disk is refused with a reason');
  check(load(0, rom) === 0, 'stand-in ROM loads');
  check(u32(x.pc_power_on(0)) === 0, 'power on');
  check(u32(x.pc_run(10000)) === 1, 'runs to HLT');
  x.pc_text();
  check(out().startsWith('A'), 'text row 0 shows the character it wrote');
  check(x.pc_frame() === 0, 'no frame for text without a character ROM');
  const sha = createHash('sha256').update(rom).digest('hex');
  x.pc_sha256(put(rom));
  check(out() === sha, 'SHA-256 matches node:crypto');
  const cmd = new TextEncoder().encode('r');
  x.pc_monitor(put(cmd));
  check(/^AX=B809 /.test(out()) && /HLT/.test(out()), 'debugger shows registers after HLT');
}

// --- ROM-gated: the RTL transcript through the WASM build.
const dir = process.env.IBM_PC_ROMS;
if (!dir || !existsSync(join(dir, 'bios.bin'))) {
  console.log('skip RTL replay: set IBM_PC_ROMS to a directory with the original images');
} else {
  const file = (n) => {
    const b = readFileSync(join(dir, n));
    if (SHA[n]) check(createHash('sha256').update(b).digest('hex') === SHA[n], `${n} is the original image`);
    return new Uint8Array(b);
  };
  check(load(0, file('bios.bin')) === 0, 'BIOS loads');
  check(load(1, file('basic.bin')) === 0, 'BASIC loads');
  check(load(3, file('pcdos100.img'), 1) === 0, 'PC DOS 1.00 in drive A (write protected)');
  check(load(4, file('workdisk.img')) === 0, 'work disk in drive B');
  const expected = readFileSync(transcriptPath, 'utf8');
  const strip = (t) => t.split('\n').filter((l) => !l.startsWith('#')).join('\n');
  const t0 = performance.now();
  const len = u32(x.pc_session(put(new TextEncoder().encode(expected))));
  const ms = performance.now() - t0;
  const got = out();
  check(len !== ERR, `session ran (${len === ERR ? got.slice(0, 200) : 'ok'})`);
  const same = strip(got) === strip(expected);
  if (!same) {
    const a = strip(got).split('\n'), b = strip(expected).split('\n');
    const i = a.findIndex((l, k) => l !== b[k]);
    console.log(`first difference at line ${i}:\n  wasm: ${a[i]}\n  rtl:  ${b[i]}`);
  }
  const steps = expected.split('\n').filter((l) => l === 'screen').length;
  check(same, `WASM transcript identical to the RTL's (${steps} steps, ${(ms / 1000).toFixed(2)} s)`);

  // Speed: one emulated second of the BIOS POST, batch by batch as the worker does.
  check(u32(x.pc_power_on(1)) === 0, 'power on with audio capture');
  const batch = Math.round(CPU_HZ / 100);
  const t1 = performance.now();
  for (let i = 0; i < 100; i++) {
    x.pc_run(batch);
    x.pc_audio(44100);
    if (i % 2 === 0) x.pc_text();
  }
  const s = (performance.now() - t1) / 1000;
  console.log(`speed: 1.00 s of 5150 time in ${s.toFixed(3)} s (${(100 / s).toFixed(0)}% of real time)`);
  check(s < 1.0, 'the WASM machine runs at least at real time in Node');
}

console.log(failures ? `${failures} FAILED` : 'all passed');
process.exit(failures ? 1 : 0);
