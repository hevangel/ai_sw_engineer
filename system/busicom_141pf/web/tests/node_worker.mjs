// Node harness that runs the browser worker script (www/js/worker.js) itself
// — the exact code the browser executes — against the built WASM module.
// Covers the load path (fetch → scratch → init) and key-event handling that
// the in-app browser test cannot reach because it does not support file
// choosers (plans/testplan.md T3 note).
//
//   node system/busicom_141pf/web/tests/node_worker.mjs
//   BUSICOM_ROM=/path/to/rom_141pf_combined.bin \
//   node system/busicom_141pf/web/tests/node_worker.mjs \
//        --schedule <schedule.txt> --trace <rom_port_trace.txt>
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const web = join(dirname(fileURLToPath(import.meta.url)), '..');
const workerSrc = readFileSync(join(web, 'www', 'js', 'worker.js'), 'utf8');
const wasmPath = join(web, 'www', 'busicom_emulator_wasm.wasm');

// The committed lamp stimulus from emulator/tests/replay.rs (no proprietary
// input): FIM P0,0x40; SRC P0; LDM 5; WMP; JUN 0.
const LAMP_PROGRAM = [0x20, 0x40, 0x21, 0xd5, 0xe1, 0x00, 0x40, 0x06];

// Instantiate the worker script with a stub `self` and a file-backed fetch.
function makeWorker() {
  const messages = [];
  const self = { postMessage: (m) => messages.push(m) };
  const fn = new Function('self', 'fetch', workerSrc);
  fn(self, async (url) => {
    if (String(url) === '../busicom_emulator_wasm.wasm') {
      const bytes = readFileSync(wasmPath);
      return new Response(bytes, { headers: { 'content-type': 'application/wasm' } });
    }
    return new Response(null, { status: 404 });
  });
  return {
    messages,
    send(msg) {
      self.onmessage({ data: msg });
    },
    // The worker handlers are async; wait for the next message of a type.
    async next(type, timeoutMs = 30000) {
      const t0 = Date.now();
      for (;;) {
        const idx = this.messages.findIndex((m) => m.type === type);
        if (idx >= 0) return this.messages.splice(idx, 1)[0];
        if (Date.now() - t0 > timeoutMs) throw new Error(`timeout waiting for ${type}`);
        await new Promise((r) => setTimeout(r, 5));
      }
    },
  };
}

async function selftest() {
  const worker = makeWorker();
  worker.send({ type: 'load', rom: Uint8Array.from(LAMP_PROGRAM).buffer });
  const loaded = await worker.next('loaded');
  if (!loaded.ok) throw new Error(`worker load failed: ${loaded.error}`);
  console.log(`ok: worker loads the machine (${loaded.build})`);

  worker.send({ type: 'key', key: 0, precision: 2, rounding: 0, halfSpins: 1 });
  const settle = await worker.next('rows');
  if (settle.rows.length !== 0) throw new Error('settle printed rows');
  console.log('ok: settle event prints nothing');

  worker.send({ type: 'lamps' });
  const lamps = await worker.next('lamps');
  if (lamps.lamps !== 5) throw new Error(`lamps ${lamps.lamps} != 5`);
  console.log('ok: worker reports lamps 5 after WMP');

  worker.send({ type: 'reset' });
  const reset = await worker.next('reset');
  if (reset.lamps !== 0) throw new Error('reset did not clear lamps');
  console.log('ok: worker reset clears the lamps');
  console.log('PASS: worker harness self-test');
}

async function replay(schedulePath, tracePath) {
  const romPath = process.env.BUSICOM_ROM;
  if (!romPath) throw new Error('set BUSICOM_ROM to the provenance-hashed original ROM');
  const rom = new Uint8Array(readFileSync(romPath)).buffer;
  const schedule = readFileSync(schedulePath, 'utf8');
  const want = readFileSync(tracePath, 'utf8').split('\n')
    .filter((l) => l !== '' && !l.startsWith('#'))
    .join('\n');

  const worker = makeWorker();
  worker.send({ type: 'load', rom });
  const loaded = await worker.next('loaded');
  if (!loaded.ok) throw new Error(`worker load failed: ${loaded.error}`);

  const payload = [];
  for (const line of schedule.split('\n')) {
    const l = line.trim();
    if (l === '' || l.startsWith('# ')) continue;
    const f = l.split(/\s+/);
    if (f[0] === '#EXAMPLE') {
      worker.send({ type: 'reset' });
      await worker.next('reset');
    } else if (f[0] === '#RESET') {
      // checkpoint only
    } else if (f[0] === '#STEP') {
      worker.send({ type: 'lamps' });
      payload.push(`LAMPS|${(await worker.next('lamps')).lamps}`);
    } else {
      worker.send({
        type: 'key', key: Number(f[2]), precision: Number(f[0]),
        rounding: Number(f[1]), halfSpins: Number(f[3]),
      });
      const rows = await worker.next('rows');
      if (rows.rows.length > 0) payload.push(...rows.rows);
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
  console.log(`PASS: worker replay matches the committed original-ROM trace (${payload.length} payload lines)`);
}

const args = process.argv.slice(2);
if (args.includes('--schedule') && args.includes('--trace')) {
  await replay(args[args.indexOf('--schedule') + 1], args[args.indexOf('--trace') + 1]);
} else {
  await selftest();
}
