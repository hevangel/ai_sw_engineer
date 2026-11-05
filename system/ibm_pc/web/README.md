# IBM PC 5150 browser frontend

The IBM PC 5150 running in the browser: the project's native Rust system emulator
([../emulator](../emulator/README.md)) compiled to WebAssembly, running in a Web
worker behind a canvas screen, keyboard, speaker, media loader and debugger. Owning
issues: hevangel/ai_sw_engineer#88 (parent), #249 (functional stage), #250 (polish).
Specification and UI/backend contract: [spec/spec.md](spec/spec.md); test plan:
[plans/testplan.md](plans/testplan.md); verification record:
[report/verification.md](report/verification.md).

**Status:** Mode A (Rust/WASM, the default) works: it boots the original IBM BIOS and
PC DOS 1.00 from user-supplied images, takes real keyboard input, plays the speaker,
reads and writes diskettes, and shows the same screens the hardware project's RTL
recorded (DATE, DIR, CHKDSK typed in the page; the full 20-step session through the
WASM exports). Mode B (the SystemVerilog machine through Xezim) is an explanatory
unavailable state: the adapter it needs (hevangel/ai_engineer#55, #80) is not
available here.

## Build and run

Needs the repo's pinned Rust (`cargo +1.99.1`) with `wasm32-unknown-unknown`, and any
static file server. No bundler, no npm tree. The built `www/ibm_pc_emulator_wasm.wasm`
is committed, so serving `www/` works without building; rebuild after an emulator change.

```bash
rustup target add --toolchain 1.99.1 wasm32-unknown-unknown
```

```bash
system/ibm_pc/web/scripts/build.sh
```

```bash
python3 -m http.server 8080 --directory system/ibm_pc/web/www
```

Open http://localhost:8080 (`file://` does not work: the worker and the WASM are
fetched). Load the BIOS (required), and optionally Cassette BASIC, the CGA character
ROM and a diskette for drive A (choose them, or drop them anywhere on the page), then
press **Power on**. The page remembers the files in this browser unless told not to. The page shows each file's
SHA-256 and labels it as an original only when it matches
[the recorded hashes](../emulator/original/provenance.md). IBM's firmware and PC DOS
are copyrighted: they are never bundled, fetched or committed.
`python3 -I system/ibm_pc/emulator/scripts/fetch_reference.py DIR` downloads them from
the pinned PCjs revisions for local use.

Click the screen to type; Ctrl+] gives the keyboard back to the page.

## Layout

| Path | Contents |
|---|---|
| [wasm/](wasm/) | `ibm_pc-emulator-wasm` cdylib: raw C ABI over the native `Machine` |
| [www/](www/) | the static site: `index.html`, `css/`, `js/main.js` (UI), `js/worker.js` (machine), `js/keys.js` (scan codes), `js/store.js` (remembered files), the built `.wasm` |
| [scripts/build.sh](scripts/build.sh) | pinned-toolchain WASM build into `www/` |
| [tests/node_replay.mjs](tests/node_replay.mjs) | Node: self-test without ROMs; with them, the RTL's 20-step session through the WASM exports, and speed |
| [tests/browser.mjs](tests/browser.mjs) | Playwright/Chromium: the real page driven as a user, screens against the RTL's |
| [plans/ux_plan.md](plans/ux_plan.md) | usability review of the functional stage and the polish plan (#250) |
| [report/screenshots/](report/screenshots/) | screenshots from the browser test runs: `functional/` (before polish), `polish/` (after) |

## Verify

```bash
cargo +1.99.1 test --manifest-path system/ibm_pc/web/wasm/Cargo.toml
```

```bash
node system/ibm_pc/web/tests/node_replay.mjs
```

```bash
IBM_PC_ROMS=DIR node system/ibm_pc/web/tests/node_replay.mjs
```

```bash
IBM_PC_EXPORT_IMAGES=DIR cargo +1.99.1 test --release --manifest-path system/ibm_pc/emulator/Cargo.toml --test boot_oracle
```

```bash
IBM_PC_ROMS=DIR node system/ibm_pc/web/tests/browser.mjs --shots /tmp/shots
```

The browser test needs Playwright with Chromium (`require('playwright')`). The
expected screens come from the RTL's transcripts, not from this build.
