# Busicom 141-PF browser frontend

The Busicom 141-PF running in the browser: the qualified native Rust system
emulator ([../emulator](../emulator/README.md)) compiled to WebAssembly,
executing in a Web worker behind a printer-tape/lamp/keypad UI. Owning issue:
hevangel/ai_sw_engineer#83. Specification and UI/backend contract:
[spec/spec.md](spec/spec.md); test plan: [plans/testplan.md](plans/testplan.md);
verification record: [report/verification.md](report/verification.md).

The SystemVerilog (Xezim) backend of issue #83 is a selector with an
explanatory unavailable state, not an implementation — see the spec's Mode B
contract.

## Build and run

Requires the repo's pinned Rust (`cargo 1.99.1`) with the `wasm32-unknown-unknown`
target, and any static file server. No bundler, no npm tree.

```sh
system/busicom_141pf/web/scripts/build.sh          # wasm -> www/ (gitignored)
cd system/busicom_141pf/web/www
py -m http.server 8080                             # or any static server
# open http://localhost:8080
```

`file://` does not work (the worker and the wasm are fetched). Then load a
firmware image with the file picker: 1–4,096 bytes of flat 4001 ROM. The page
shows the image's SHA-256 and labels it `original Busicom firmware` only when
it matches the recorded provenance hash
([../../firmware/original/provenance.md](../../firmware/original/provenance.md)).
Firmware is user-provided and never bundled or committed — the original
firmware is © Busicom/Intel, non-commercial use via the Replication Project.

## Layout

| Path | Contents |
|---|---|
| [wasm/](wasm/) | `busicom-emulator-wasm` cdylib: raw C ABI over `Emulator` (no wasm-bindgen) |
| [www/](www/) | static UI: `index.html`, `css/`, `js/{main,worker,keys}.js` |
| [tests/node_replay.mjs](tests/node_replay.mjs) | Node checks: always-on self-test + ROM-gated 42-example replay vs the committed original-ROM trace |
| [scripts/build.sh](scripts/build.sh) | pinned-toolchain WASM build into `www/` |

## Verify

```sh
node system/busicom_141pf/web/tests/node_replay.mjs           # self-test (no ROM)
BUSICOM_ROM=/path/to/rom_141pf_combined.bin \
  system/busicom_141pf/platform/scripts/build_schedule.py \
  system/busicom_141pf/firmware/oracle/manual_examples.json \
  -o system/busicom_141pf/firmware/build/schedule.txt && \
BUSICOM_ROM=/path/to/rom_141pf_combined.bin \
  node system/busicom_141pf/web/tests/node_replay.mjs \
    --schedule system/busicom_141pf/firmware/build/schedule.txt \
    --trace system/busicom_141pf/firmware/oracle/rom_port_trace.txt
```

The replay compares the WASM machine against the committed trace of the
original firmware — not against itself. Browser interaction checks are in the
test plan and were executed as recorded in the verification report.
