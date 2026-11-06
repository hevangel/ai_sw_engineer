// Browser checks of the real page in headless Chromium (Playwright).
//
//   IBM_PC_ROMS=DIR node system/ibm_pc/web/tests/browser.mjs [--shots DIR]
//
// Serves www/ on a local port, then drives the page as a user would: error
// states, loading the original images through the file inputs, power on,
// typing on the keyboard into the focused screen and through the text box,
// pause/resume, Ctrl+Alt+Del, downloading drive B, the debugger, the
// SystemVerilog backend's unavailable state, and a narrow layout. The
// screens are compared with the ones the hardware project's RTL recorded
// (../../emulator/oracle/transcripts/dos100_session.txt), not with this
// build's own output. Also checks console errors, network requests (only
// this origin) and main-thread responsiveness while the machine runs.
import { createServer } from 'node:http';
import { readFileSync, existsSync, mkdirSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { dirname, join, extname, normalize } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const { chromium } = require('playwright');

const here = dirname(fileURLToPath(import.meta.url));
const www = join(here, '../www');
const transcript = readFileSync(join(here, '../../emulator/oracle/transcripts/dos100_session.txt'), 'utf8');
const roms = process.env.IBM_PC_ROMS;
const shotsArg = process.argv.indexOf('--shots');
const shots = shotsArg > 0 ? process.argv[shotsArg + 1] : null;
if (shots) mkdirSync(shots, { recursive: true });
if (!roms || !existsSync(join(roms, 'bios.bin'))) {
  console.log('skip: set IBM_PC_ROMS to a directory with the original images');
  process.exit(0);
}

let failures = 0;
const check = (ok, what) => {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${what}`);
  if (!ok) failures++;
};

// Screens the RTL recorded, in order: the 25 rows after each step.
function rtlScreens() {
  const lines = transcript.split('\n');
  const out = [];
  for (let i = 0; i < lines.length; i++) {
    if (lines[i] === 'screen') out.push(lines.slice(i + 1, i + 26).map((l) => l.slice(1)));
  }
  return out;
}
const RTL = rtlScreens();

const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.wasm': 'application/wasm' };
const server = createServer((req, res) => {
  const path = normalize(decodeURIComponent(new URL(req.url, 'http://x').pathname)).replace(/^(\.\.[/\\])+/, '');
  const file = join(www, path === '/' ? 'index.html' : path);
  if (!file.startsWith(www) || !existsSync(file)) { res.writeHead(404); res.end(); return; }
  res.writeHead(200, { 'content-type': TYPES[extname(file)] || 'application/octet-stream' });
  res.end(readFileSync(file));
});
await new Promise((r) => server.listen(0, '127.0.0.1', r));
const origin = `http://127.0.0.1:${server.address().port}`;

const browser = await chromium.launch();
const context = await browser.newContext({ viewport: { width: 1280, height: 1000 }, deviceScaleFactor: shots ? 2 : 1, acceptDownloads: true });
const page = await context.newPage();
const consoleErrors = [];
const requests = [];
page.on('console', (m) => { if (m.type() === 'error') consoleErrors.push(m.text()); });
page.on('pageerror', (e) => consoleErrors.push(String(e)));
context.on('request', (r) => requests.push(r.url()));

const shot = async (name, p = page) => { if (shots) await p.screenshot({ path: join(shots, `${name}.png`), fullPage: true }); };
const text = (sel) => page.locator(sel).innerText();

// Read the PC's text screen through the page's own "Screen as text" view
// (it does not pause the machine; debugger commands do).
async function pcScreen() {
  const n = await page.evaluate(() => window.__pcStats.texts);
  await page.click('#text-refresh');
  await page.waitForFunction((k) => window.__pcStats.texts > k, n);
  const rows = (await page.locator('#text-out').textContent()).split('\n');
  while (rows.length < 25) rows.push('');
  return rows.slice(0, 25);
}
async function waitScreen(pred, what, ms = 60000) {
  const t0 = Date.now();
  let rows = [];
  while (Date.now() - t0 < ms) {
    rows = await pcScreen();
    if (pred(rows)) return rows;
    await page.waitForTimeout(250);
  }
  console.log(rows.join('\n'));
  check(false, `timed out waiting for ${what}`);
  return rows;
}
const same = (a, b) => a.length === b.length && a.every((r, i) => r.trimEnd() === b[i].trimEnd());

await page.goto(origin);
await page.waitForFunction(() => document.getElementById('build').textContent.includes('ibm_pc-emulator-wasm'));
check(true, 'page loads and the worker reports its build');
check(await page.locator('#backend-wasm').isChecked(), 'Rust/WASM is the default backend');
await shot('01_initial');

// Error and empty states.
check(await page.locator('#power').isDisabled(), 'Power on is disabled until a BIOS is loaded');
check(/Load the IBM BIOS ROM/.test(await text('#overlay')), `the screen says what is missing: "${await text('#overlay-text')}"`);
await page.setInputFiles('#f-bios', { name: 'wrong.bin', mimeType: 'application/octet-stream', buffer: Buffer.alloc(100) });
await page.waitForFunction(() => /not loaded:/.test(document.getElementById('s-0').textContent));
check(/8,192/.test(await text('#s-0')), 'a wrong-size BIOS is refused with the size it needs');
await page.setInputFiles('#f-a', { name: 'junk.img', mimeType: 'application/octet-stream', buffer: Buffer.alloc(1000) });
await page.waitForFunction(() => /not loaded:/.test(document.getElementById('s-3').textContent));
check(/163,840/.test(await text('#s-3')), 'a wrong-size disk is refused with the size it needs');
await shot('02_errors');

// Load the originals.
await page.setInputFiles('#f-bios', join(roms, 'bios.bin'));
await page.setInputFiles('#f-basic', join(roms, 'basic.bin'));
await page.setInputFiles('#f-font', join(roms, 'cga.bin'));
await page.setInputFiles('#f-a', join(roms, 'pcdos100.img'));
await page.click('#blank-b');
for (const [slot, name] of [[0, 'BIOS'], [1, 'BASIC'], [2, 'character ROM'], [3, 'PC DOS 1.00']]) {
  await page.waitForFunction((s) => /original:/.test(document.getElementById(`s-${s}`).textContent), slot);
  check((await text(`#s-${slot}`)).includes(name), `slot ${slot} recognised as the original ${name}`);
}
await page.waitForFunction(() => /blank-disk|blank disk/.test(document.getElementById('s-4').textContent));
check(/user-provided/.test(await text('#s-4')), 'the generated blank disk is not labelled as an original');
await shot('03_loaded');

// Boot fast, then real time.
check(/Ready/.test(await text('#overlay-text')), 'the screen says the machine is ready');
await page.selectOption('#speed', 'max');
await page.click('#power');
await page.locator('#textview summary').click();
await page.locator('#debug summary').click();
const date = await waitScreen((r) => r.join('\n').includes("Enter today's date"), 'the DOS date prompt');
check(same(date, RTL[0]), 'boot screen identical to the RTL\'s (step 1)');
await page.selectOption('#speed', 'real');
await shot('04_date_prompt');

// Type on the keyboard into the focused screen.
await page.locator('#screen').focus();
check(/Keyboard → PC/.test(await text('#capture')), 'the keyboard-capture badge shows keys go to the PC');
await page.keyboard.type('10-14-81');
await page.keyboard.press('Enter');
const prompt = await waitScreen((r) => r.some((l) => l.trimEnd().endsWith('A>')), 'the A> prompt');
check(same(prompt, RTL[1]), 'screen after typing the date is identical to the RTL\'s (step 2)');
await page.locator('#screen').focus();
await page.keyboard.type('dir');
await page.keyboard.press('Enter');
const dir = await waitScreen((r) => r.join('\n').includes('COMM      BAS') && r[24].trimEnd().endsWith('A>'), 'DIR to finish');
check(same(dir, RTL[2]), 'DIR listing identical to the RTL\'s (step 3)');
await shot('05_dir');

// Main-thread responsiveness while running in real time: long tasks over 2 s.
const longTasks = await page.evaluate(() => new Promise((resolve) => {
  const seen = [];
  const obs = new PerformanceObserver((l) => { for (const e of l.getEntries()) seen.push(e.duration); });
  obs.observe({ entryTypes: ['longtask'] });
  setTimeout(() => { obs.disconnect(); resolve(seen); }, 2000);
}));
check(longTasks.filter((d) => d > 100).length === 0, `no main-thread task over 100 ms while running (${longTasks.length} long tasks, max ${Math.max(0, ...longTasks).toFixed(0)} ms)`);
const frames0 = await page.evaluate(() => window.__pcStats.frames);
await page.waitForTimeout(1000);
const fps = (await page.evaluate(() => window.__pcStats.frames)) - frames0;
check(fps >= 20 && fps <= 65, `screen updates ${fps} times per second (CGA refresh is 60 Hz)`);
const status = await text('#status');
const pct = Number((status.match(/(\d+)% of a real/) || [])[1]);
check(pct >= 90 && pct <= 110, `status reports real-time speed: "${status}"`);

// The text box types too (CHKDSK, against the RTL's screen).
await page.fill('#type-text', 'chkdsk');
await page.click('#type-send');
const chk = await waitScreen((r) => r.join('\n').includes('bytes free') && r[24].trimEnd().endsWith('A>'), 'CHKDSK');
check(same(chk, RTL[3]), 'CHKDSK screen identical to the RTL\'s (step 4)');

// Write to drive B and download it.
await page.fill('#type-text', 'copy time.com b:clock.com');
await page.click('#type-send');
await waitScreen((r) => r.join('\n').includes('1 File(s) copied'), 'COPY to drive B');
const [download] = await Promise.all([page.waitForEvent('download'), page.click('#save-b')]);
const disk = readFileSync(await download.path());
const root = disk.subarray(3 * 512, 7 * 512).toString('latin1');
check(disk.length === 163840 && root.includes('CLOCK   COM'), 'downloaded drive B has CLOCK.COM in its directory');
const timeCom = readFileSync(join(roms, 'pcdos100.img')).subarray(3 * 512, 7 * 512).toString('latin1');
check(timeCom.includes('TIME    COM'), '(TIME.COM is on the original DOS disk)');

// Pause / resume.
await page.click('#pause');
await page.waitForFunction(() => document.getElementById('state').textContent === 'Paused');
check(await page.locator('#overlay').isVisible() && /Paused/.test(await text('#overlay-text')), 'the screen shows the paused state');
const c1 = await page.evaluate(() => document.getElementById('status').textContent);
await page.waitForTimeout(600);
check(c1 === await text('#status'), 'paused: the machine clock stands still');
const r = await (async () => { await page.click('button[data-cmd="r"]'); await page.waitForTimeout(200); return text('#mon-out'); })();
check(/AX=[0-9A-F]{4} BX=/.test(r), 'debugger shows registers');
await shot('06_debugger');
await page.click('#pause');
await page.waitForFunction(() => document.getElementById('state').textContent === 'Running');
check(true, 'resumed');

// Keyboard: Ctrl+] gives focus back to the page; Tab then moves on.
await page.locator('#screen').focus();
await page.keyboard.press('Control+BracketRight');
check(await page.evaluate(() => document.activeElement.id) === 'type-text', 'Ctrl+] releases the keyboard to the text box');
check(/Click the screen/.test(await text('#capture')), 'and the badge says so');

// Special-key row: Esc on the DOS prompt cancels the line ("\\" and a new line).
await page.fill('#type-text', 'xyz');
await page.uncheck('#type-enter');
await page.click('#type-send');
await page.check('#type-enter');
await page.click('.keys button[data-key="0x01"]');
const esc = await waitScreen((r) => r.some((l) => l.trimEnd() === 'A>xyz\\'), 'Esc to cancel the line');
check(esc.some((l) => l.trimEnd() === 'A>xyz\\'), 'the Esc button reaches DOS (line cancelled)');
await page.click('.keys button[data-key="0x1C"]');

// Sound: the beep at the next power-on reaches Web Audio.
await page.check('#sound');
await page.selectOption('#speed', 'real');
await page.click('#power');
await page.waitForFunction(() => window.__pcStats.audioChunks > 0, null, { timeout: 30000 });
check(true, 'speaker samples are delivered to Web Audio');

// Ctrl+Alt+Del after the restart reaches the BIOS (it reboots: screen clears to the POST).
await page.selectOption('#speed', 'max');
await waitScreen((r) => r.join('\n').includes("Enter today's date"), 'the date prompt after restart');
await page.fill('#type-text', '10-14-81');
await page.click('#type-send');
await waitScreen((r) => r.some((l) => l.trimEnd() === 'A>'), 'A> after restart');
await page.click('#cad');
await waitScreen((r) => r.join('\n').includes("Enter today's date") && !r.some((l) => l.includes('A>')),
  'a fresh date prompt after Ctrl+Alt+Del');
check(true, 'Ctrl+Alt+Del reboots to DOS');
const [download2] = await Promise.all([page.waitForEvent('download'), page.click('#save-b')]);
const disk2 = readFileSync(await download2.path());
check(disk2.subarray(3 * 512, 7 * 512).toString('latin1').includes('CLOCK   COM'), 'drive B keeps CLOCK.COM across the power cycle');

// Compiled software: the Rust hello_rs floppy built by llvm-i8086 (export it
// with IBM_PC_EXPORT_IMAGES=$IBM_PC_ROMS cargo test --test boot_oracle), booted
// from drive A; the screen must be the one the RTL recorded for that image.
const helloImg = join(roms, 'hello_rs.img');
if (existsSync(helloImg)) {
  const oracle = readFileSync(join(here, '../../emulator/oracle/boot/hello_rs.txt'), 'utf8');
  const sha = createHash('sha256').update(readFileSync(helloImg)).digest('hex');
  check(oracle.includes(`# image sha256 ${sha}`), 'hello_rs.img is the image the RTL booted');
  const want = oracle.split('\n').filter((l) => l.startsWith('|')).map((l) => l.slice(1));
  await page.setInputFiles('#f-a', helloImg);
  await page.waitForFunction(() => /hello_rs/.test(document.getElementById('s-3').textContent));
  await page.click('#power');
  const got = await waitScreen((r) => r.some((l) => l.trimEnd() === 'done'), 'hello_rs to finish');
  check(same(got, want), 'Rust hello_rs (llvm-i8086) screen identical to the RTL\'s');
  await shot('07a_hello_rs');
} else {
  console.log('skip hello_rs: no hello_rs.img in IBM_PC_ROMS');
}

// The SystemVerilog backend: explanatory unavailable state, then back.
await page.check('#backend-sv');
check(await page.locator('#sv-panel').isVisible(), 'SV backend shows the unavailable panel');
check(/not qualified or available/.test(await text('#sv-panel')), 'it says why');
await shot('07_sv_unavailable');
await page.click('#sv-back');
check(await page.locator('#wasm-ui').isVisible(), 'back to the Rust emulator');
check(/Machine off/.test(await text('#status')) && await text('#state') === 'Off', 'the backend switch reset the machine');

// Remembered media: a reload restores the files chosen earlier.
await page.reload();
await page.waitForFunction(() => /remembered/.test(document.getElementById('s-0').textContent), null, { timeout: 10000 });
check(/original: IBM 5150 BIOS/.test(await text('#s-0')), 'after a reload the BIOS is restored from this browser');
check(!(await page.locator('#power').isDisabled()), 'and Power on is available at once');
await page.click('#forget');
await page.waitForFunction(() => document.getElementById('forget').textContent === 'Forgotten');
await page.reload();
await page.waitForFunction(() => document.getElementById('build').textContent.length > 0);
await page.waitForTimeout(500);
check(/not loaded/.test(await text('#s-0')), 'Forget clears the remembered files');

// Drag and drop: files are assigned by recorded hash.
const dropFiles = [['basic.bin', readFileSync(join(roms, 'basic.bin'))], ['bios.bin', readFileSync(join(roms, 'bios.bin'))]];
const dt = await page.evaluateHandle((files) => {
  const d = new DataTransfer();
  for (const [name, data] of files) d.items.add(new File([new Uint8Array(data)], name));
  return d;
}, dropFiles.map(([n, b]) => [n, [...b]]));
await page.dispatchEvent('body', 'drop', { dataTransfer: dt });
await page.waitForFunction(() => /original/.test(document.getElementById('s-0').textContent) && /original/.test(document.getElementById('s-1').textContent));
check(true, 'dropped BIOS and BASIC land in their slots');

// Narrow layout.
await page.setViewportSize({ width: 390, height: 844 });
const overflow = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
check(overflow <= 0, `no horizontal scroll at 390 px (${overflow} px)`);
await shot('08_narrow');
await page.emulateMedia({ colorScheme: 'dark' });
await page.setViewportSize({ width: 1280, height: 1000 });
await shot('09_dark');

check(consoleErrors.length === 0, `no console errors${consoleErrors.length ? ': ' + consoleErrors.join(' | ') : ''}`);
const foreign = requests.filter((u) => !u.startsWith(origin) && !u.startsWith('blob:') && !u.startsWith('data:'));
check(foreign.length === 0, `no requests outside this origin (${requests.length} requests)${foreign.length ? ': ' + foreign.join(', ') : ''}`);

await browser.close();
server.close();
console.log(failures ? `${failures} FAILED` : 'all passed');
process.exit(failures ? 1 : 0);
