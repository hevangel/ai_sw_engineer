# Browser frontend verification record — Busicom 141-PF

Owning issue: hevangel/ai_sw_engineer#83 (Mode A). Recorded 2026-10-25/26
(America/Vancouver). Test plan: [../plans/testplan.md](../plans/testplan.md).
Build: pinned `cargo 1.99.1`, target `wasm32-unknown-unknown`,
`busicom_emulator_wasm.wasm` = **57,584 bytes**,
SHA-256 `6cf9284804b05347d6867371dd3a40cdeca12f226aa9e777b2a047e47a6db212`
(the artifact itself is gitignored and rebuilt by
[../scripts/build.sh](../scripts/build.sh)). `cargo fmt -- --check` and
`cargo clippy --all-targets -- -D warnings` pass for the binding crate.

## T1 — WASM binding self-test (no ROM) — PASS

`node tests/node_replay.mjs`: 21/21 checks — all documented exports exist,
build id identifies the binding, empty image fails with a message, the
committed 8-byte lamp program drives `lamps` to 5 and reset clears it, dp 7 /
rounding 2 / keys 128 and 161 are rejected exactly like the native API.

## T2 — Full replay vs committed original-ROM trace (ROM-gated) — PASS

Schedule: `platform/scripts/build_schedule.py` over
`firmware/oracle/manual_examples.json` (1,196 lines). Trace:
`firmware/oracle/rom_port_trace.txt` (600 ROW/LAMPS payload lines, the
committed capture of the original firmware on the independent board model;
the native release CLI reproduces it line-for-line).

- `node tests/node_replay.mjs --schedule … --trace …` with `BUSICOM_ROM`:
  **PASS — 600/600 payload lines byte-identical** through the WASM exports.
- `node tests/node_worker.mjs --schedule … --trace …` (runs the actual
  `www/js/worker.js` — fetch → scratch → init → key events):
  **PASS — 600/600 payload lines byte-identical**; self-test 4/4 (load,
  settle, lamps, reset).

## T3 — Real browser interaction — PASS (with a recorded runtime limit)

Automated pass in the ZCode in-app browser (Chromium) against
`py -m http.server 8093` serving `www/`, on 2026-10-25:

- Page loads with title "Busicom 141-PF — browser machine"; DOM snapshot
  shows the backend selector (Rust/WASM default), file input, disabled
  Power/reset, three lamp indicators, printer tape, Decimal/Rounding
  selectors and all 30 keypad keys in the physical layout. No visible error
  manifestations; the status line stayed clean. Console-log capture is not
  exposed by this runtime, so absence of errors was judged from the page's
  own error surfaces (status line, intact rendering).
- Backend selector → `SystemVerilog (Xezim)`: the explanatory unavailable
  panel appears (three pending dependencies named) and the default machine
  stays visible and active — the status line reads "SystemVerilog backend
  unavailable — default Rust/WASM machine stays active". Screenshot captured.
  Switching back hides the panel.
- Keyboard input with no ROM loaded is a silent no-op (by design; the power
  path requires firmware) — no crash, no error state. Power/reset stays
  disabled until an image is loaded.
- Narrow layout (375 px viewport): no horizontal overflow
  (`scrollWidth == clientWidth == 360`), keypad renders 4 columns with usable
  key sizes; screenshot captured. Desktop layout re-verified after restore.
- **Runtime limit (recorded, not a product bug):** the in-app browser does
  not support file choosers, so the file-picker flow (provenance label,
  calculator runs, power-cycle with the original ROM) could not be clicked
  through in this browser. That flow is covered at the code level by the
  worker harness above — the same worker script, the same WASM artifact and
  the same ROM replay 600/600 — and remains open for a human click-through
  or an upload-capable runtime.

## T4 — Static-hosting contract — PASS

`www/` is plain HTML/CSS/ES modules (no bundler output, no analytics, no
remote calls; the only runtime fetches are the page's own assets and the
gitignored wasm artifact). Served and exercised with `py -m http.server`
only; `file://` documented as unsupported.

## T5 — Reproducible build — PASS

`scripts/build.sh` builds with `cargo +1.99.1 --release --target
wasm32-unknown-unknown` and copies the artifact to `www/`. `cargo fmt
-- --check` and `cargo clippy --all-targets -- -D warnings` pass for the
binding crate (run 2026-10-25; `external/chips` has pre-existing warnings,
unchanged by this work).

## Limits

- Mode B (Xezim/SystemVerilog) is a selector with an explanatory unavailable
  state — by scope, not a failure ([../spec/spec.md](../spec/spec.md) Mode B).
- Paper-advance control: not exposed by the emulator API (spec limitation).
- The file-picker browser flow is code-verified (worker harness) but not yet
  human-clicked; see the T3 runtime note.
