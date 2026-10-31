# Busicom 141-PF browser frontend — test plan

Owning issue: hevangel/ai_sw_engineer#83. Every check is run and recorded in
[../report/verification.md](../report/verification.md); checks that were not
run are reported as not run.

## T1 — WASM binding, always-on (no ROM required)

`tests/node_replay.mjs --selftest` under Node:

- all documented exports exist on the module;
- `busicom_init(0)` fails with the message in the output buffer;
- the committed 8-byte lamp program from the emulator unit tests
  (`FIM/SRC/LDM/WMP`, no proprietary input) drives `busicom_advance` +
  `busicom_lamps` to the same `LAMPS|5` result, and `busicom_reset` clears it;
- `busicom_advance` rejects the same invalid inputs the native API rejects
  (dp 7, rounding 2, key 143).

## T2 — WASM full replay vs committed original-ROM trace (ROM-gated)

With `BUSICOM_ROM` pointing at the provenance-hashed original image and a
schedule built by [../../platform/scripts/build_schedule.py](../../platform/scripts/build_schedule.py):

- [tests/node_replay.mjs](../tests/node_replay.mjs) `--schedule … --trace …`
  instantiates the WASM module directly, replays all 42 manual examples
  through the exports and requires the payload to equal the committed trace
  [../../firmware/oracle/rom_port_trace.txt](../../firmware/oracle/rom_port_trace.txt)
  byte for byte. That trace was recorded from the native emulator against the
  original firmware — the WASM path is compared to it, not to itself.
- [tests/node_worker.mjs](../tests/node_worker.mjs) runs the actual worker
  script (`www/js/worker.js`) under Node — the same code the browser executes,
  including the fetch → scratch → init path — and repeats the replay through
  the worker message protocol.

## T3 — Real browser interaction (Mode A acceptance)

Served from a static file server, driven with an automated browser:

- the page loads with no console errors and no unexpected network requests;
- loading the original ROM file shows its size, SHA-256 and the
  `original Busicom firmware` provenance label;
- pressing `1`, `+`, `2`, `=` (pointer clicks) prints the expected tape rows
  and updates the lamps; typing the same sequence on the PC keyboard does the
  same;
- reset/power-cycle clears tape and lamps;
- selecting the `SystemVerilog (Xezim)` backend shows the explanatory
  unavailable state and the default machine keeps working without any SV
  bridge;
- desktop and narrow layouts render usable; capture screenshots.

Runtime note (2026-10-25): the available automation browser is the ZCode
in-app browser, which does not support file choosers. The file-picker points
(provenance label, pointer/keyboard calculator runs, power-cycle) therefore
cannot be exercised inside it; they are covered at the code level by the
worker harness (T2's `node_worker.mjs` runs the real worker script with the
real ROM) plus the no-ROM interaction checks (page load, keypad, keyboard
no-op, backend selector, layouts) that the browser pass does execute. A
human-run pass through the file picker remains open until a runtime with
upload support or a human tester is available.

## T4 — Static-hosting contract

- `www/` contains no bundler output, no analytics, no remote calls: audit the
  page's network traffic during T3 (only local page assets + the wasm).
- Documented launch command (`py -m http.server`) reproduces T3 locally.

## T5 — Reproducible build

- [../scripts/build.sh](../scripts/build.sh) builds the WASM with the pinned
  toolchain (`cargo +1.99.1`, target `wasm32-unknown-unknown`) and copies the
  artifact into `www/`; record artifact size and SHA-256.
- `cargo fmt --check` and `cargo clippy -- -D warnings` pass for the binding
  crate.

## Non-goals for this stage

- Mode B execution (contract only, see [../spec/spec.md](../spec/spec.md)).
- Cross-backend snapshots, waveform capture, paper-advance control.
