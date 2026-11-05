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

## 2026-11-05: UI/UX polish stage (issue #250)

Baseline: the functional stage above (commit `84cf934`). Its evidence was reviewed by
the agent before this stage started (35/35 browser checks, RTL-identical screens,
screenshots inspected); **human acceptance of #249 has not been given** and is not
claimed. Review findings U1-U12 and the plan are in
[../plans/ux_plan.md](../plans/ux_plan.md). The WASM artifact is unchanged by this
stage (same SHA-256 as above); only `www/` HTML, CSS and JS changed, plus one worker
message (`text`) and the `stopped` flag on errors.

| Finding | Resolution | Evidence |
|---|---|---|
| U1 screen below the fold | two columns (monitor left); one column with the monitor first below 960 px | [before](screenshots/functional/05_dir.png) / [after](screenshots/polish/05_dir.png); [390 px before](screenshots/functional/08_narrow.png) / [after](screenshots/polish/08_narrow.png) |
| U2 unexplained black screen, stale frame after a backend switch | overlay for loading / missing BIOS / ready / paused / stopped; the canvas is cleared on a switch | [before](screenshots/functional/01_initial.png) / [after](screenshots/polish/01_initial.png); browser checks "the screen says what is missing", "ready", "paused" |
| U3 Power on enabled without a BIOS | disabled until a BIOS loads; the overlay says why | browser check "Power on is disabled until a BIOS is loaded" |
| U4 key capture invisible | badge under the screen: "Keyboard → PC · Ctrl+] to release" / "Click the screen to type" | browser checks on the badge before and after Ctrl+] |
| U5 raw inputs, hash noise, no drive activity | media cards with original/user badges (hash in tooltip), drag-and-drop by hash or size, drive motor lights | browser check "dropped BIOS and BASIC land in their slots"; [after](screenshots/polish/05_dir.png) (drive A light on during DIR) |
| U6 files chosen again on every visit | remembered in IndexedDB (W-A4) with Forget | browser checks "after a reload the BIOS is restored", "Forget clears" |
| U7 no Esc/Tab/F-keys on phones | special-key row | browser check "the Esc button reaches DOS (line cancelled)" |
| U8 screen text not accessible | "Screen as text" with copy | the browser test now reads every screen through it |
| U9 debugger changes state under the user | commands pause first; Clear and Help | [debugger](screenshots/polish/06_debugger.png); check "debugger shows registers" |
| U10 SV mode hidden in a select | segmented control labelled "unavailable" | [after](screenshots/polish/07_sv_unavailable.png) |
| U11, U12 weak hierarchy, mixed status | monitor bezel, tokens with light and dark themes, primary button, state chip + time + speed | [dark](screenshots/polish/09_dark.png) |

Regressions rerun after the polish (same environment as above):

| Command | Result |
|---|---|
| `cargo test --release --no-fail-fast -- --include-ignored` (emulator) | 80 passed; the same 2 environmental failures as the emulator's sixteenth record (stress objects and the silicon corpus are not in this container) |
| `cargo test`, `fmt --check`, `clippy -D warnings` (wasm crate) | 4 of 4; clean |
| `IBM_PC_ROMS=DIR node tests/node_replay.mjs` | all pass: the RTL's 20-step session identical through the WASM exports; one machine second in 0.033 s |
| `IBM_PC_ROMS=DIR node tests/browser.mjs --shots ...` | **45 of 45** (the 35 functional checks, adapted to the new controls, plus 10 for the polish): reset, input, media, Ctrl+Alt+Del, worker execution at 100% real time, 53 frames per second, no main-thread task over 100 ms, the SV unavailable state, 390 px without horizontal scroll, no console errors, same-origin requests only |

Remaining gaps: Mode B (Xezim signals and waveforms) is still unavailable, so there is
no waveform inspection to regress; only Chromium was exercised; sound was checked as
delivered samples, not by ear; no screen-reader session was run (the text view and
labels were checked by markup and the tests); human review of both stages is pending.
