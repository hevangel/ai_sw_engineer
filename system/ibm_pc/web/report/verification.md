# IBM PC 5150 browser frontend verification

## 2026-11-05: functional stage (issue #249)

Environment: Linux container, `cargo 1.99.1` (wasm32-unknown-unknown), Node 22.23.4,
Playwright 1.63.0 with its Chromium (headless). Original images fetched with
`../emulator/scripts/fetch_reference.py` (all SHA-256 equal to
`../emulator/original/provenance.md`). WASM artifact: 141 KB,
SHA-256 `78097d71e72f42d4029646637bc51e9d73990842a2b3c4f77d78a077f2ad2a59`, built by `scripts/build.sh` from the emulator at this commit.

| Command | Result |
|---|---|
| `cargo test` (wasm crate), `cargo fmt -- --check`, `cargo clippy --all-targets -- -D warnings` | 4 of 4; clean |
| `node tests/node_replay.mjs` | self-test: all pass without any IBM file |
| `IBM_PC_ROMS=DIR node tests/node_replay.mjs` | **the RTL's 20-step DOS and BASIC session through the WASM exports: identical** (screens, video memory in both CGA graphics modes, drive B after FORMAT and COPY), 3.2 s; one machine second in about 0.03-0.05 s (20-30 times real time in Node) |
| `IBM_PC_ROMS=DIR node tests/browser.mjs --shots ...` | **35 of 35**: refusals name the missing BIOS and the sizes needed; the four originals recognised; boot, DATE typed with key events, DIR, and CHKDSK through the text box **identical to the RTL's screens** (steps 1-4); the Rust `hello_rs` floppy built by `llvm-i8086` shows **the RTL's screen** for that image (same SHA-256); COPY to a generated blank disk B, downloaded with `CLOCK   COM` in its directory, kept across a power cycle; pause holds the clock; Ctrl+Alt+Del warm-boots; real time measured at 100%; 55 frames per second; no main-thread task over 100 ms; speaker samples reach Web Audio; the SV backend shows its unavailable state and switching back leaves the machine off; no horizontal scroll at 390 px; no console errors; 6 requests, all to the page's origin |

Findings fixed during this stage:

- A blank data disk with a zero-filled directory makes PC DOS 1.00 `COPY` fail with
  "File creation error" (it counts entries starting 00h as files, so the directory is
  full). The generated blank disk now has E5h entries (spec W-A1).
- A power cycle rebuilt the drives from the last *loaded* images, discarding the
  machine's writes; it now keeps them, as real diskettes do.
- Frames were posted on every 4 ms tick (182 per second); now at most 60 per second.

Screenshots (real runs, Chromium): [errors](screenshots/functional/02_errors.png),
[DIR typed in the page](screenshots/functional/05_dir.png),
[Rust hello_rs](screenshots/functional/07a_hello_rs.png),
[SV unavailable](screenshots/functional/07_sv_unavailable.png),
[390 px](screenshots/functional/08_narrow.png).

Remaining limits: Mode B is not implemented (no Xezim adapter here); timing is the
native emulator's instruction-level model (E-A2), so speed "100%" means machine clocks
per second, not cycle-exact behaviour; sound was checked as delivered samples, not by
listening; only Chromium was exercised; no visual design work was done (issue #250).
