# Busicom 141-PF firmware — report

Status at milestone M3 (2026-10-18).

| Check | Result |
|---|---|
| Host replay vs original ROM (recovered-ROM profile) | **42/42** examples, all 135 checkpoints |
| **Port-level replay** (M3 drivers + board model) | **42/42** examples via `tests/port_level.rs` |
| **Exact full-tape replay** vs original-ROM port trace | **42/42** examples via `tests/port_trace.rs` — every printed row (value + both symbol columns + red) and the full 4-bit RAM1 lamp nibble per step equal `oracle/rom_port_trace.txt`, no relaxations |
| Original ROM on `external/chips` through the same port wiring | **42/42** (board-model validation; `oracle/port_level_validation.md`) |
| Unit tests (BCD + 8-key buffer + lamps) | 12/12 |
| `cargo fmt --check`, `clippy -D warnings` | clean |
| `scripts/run_all.sh` (fmt, clippy, test, host build + IR) | clean (target/sim stages skipped — `llvm-i4004` is M4) |
| Target image size vs 1280-byte ROM | n/a — `llvm-i4004` backend not started (M4) |
| Primary simulator (ai_hw_engineer RTL) | n/a — needs the backend (M4/M5) |
| Secondary emulator (`external/busicom`) | validated as port oracle via `external/chips` (shared dep) |

Toolchain: `ai-sw-engineer:latest` Docker image.

## Host size of the library (M3)

The portable logic + drivers compiled for the host (`cargo build --release`):

| Artifact | Size |
|---|---|
| `libsoftware.rlib` (release) | ~270 KB (includes metadata, not code size) |
| `software` lib compiled to one object (`--emit=obj`, release) | 27,036 bytes (was 25,204 before the exact-tape work: +1,832 for the two-column row model, the A6/A7 echo logic and the bit-3 lamp) |
| Host LLVM IR (`build/host/firmware.ll`) | 117,181 bytes / 2,115 lines (was 111,958 / 2,086) |

These are **host** (x86-64) figures. The 4004 ROM size is the figure that
matters against the 1280 / 4096-byte budgets, and it **awaits the `llvm-i4004`
backend (M4)**; `build.sh` reports it against both budgets once the backend
emits an image.

## What M3 added

- **Port-level HAL** (`src/hal.rs`): four port operations (`rom_port_write`,
  `rom_port_read`, `ram_port_write`) and the `TEST` pin, with the chip-select
  and bit assignment (4003 clocks/data on ROM0, keyboard columns on ROM1, drum
  index / paper button on ROM2, printer red/hammer/advance on RAM0, lamps on
  RAM1) from the ai_hw_engineer board spec §3–4.
- **Drivers** (`src/driver_keyboard.rs`, `src/driver_printer.rs`,
  `src/driver_lamps.rs`): keyboard 4003 scan with the 8-key input buffer and
  selector reads; the drum-printer `Printer` that sequences the 13 drum sectors
  on `TEST`, shifts the 20-bit hammer word into the 4003 #1+#2 chain and fires
  hammers on the RAM0 edge, with red ink and paper advance; lamp output on RAM1.
  `src/main.rs` wires them into the target run loop (generic over `Hal`).
- **Host board model + port-level replay** (`tests/port_level.rs`): a board
  model implementing `Hal` (4003 shift registers, keyboard matrix, drum/TEST
  generator, hammer/lamp decode) replays the 42 examples through
  `keyboard scan → Calculator → printer drum → board decode`, reaching the same
  42/42 as the core. Ratchet `oracle/port_level_pass.txt`.
- **Board-model validation** against the original ROM: the unmodified 1280-byte
  ROM on `external/chips` (pinned `1bc3d57…`) through the documented board wiring
  scores 42/42 at the port level; the trace is committed as
  `oracle/rom_port_trace.txt` with provenance (no ROM binary), method in
  `oracle/port_level_validation.md`.
- **Exact full-tape reproduction** (`tests/port_trace.rs`): the core now prints
  the ROM's complete tape — the `×`/`÷` and `M+`/`M-` operand echoes (A6/A7),
  two independent symbol columns via `print::Row { sym_a, sym_b }` (A8), and the
  minus-sign lamp from WR's sign plus RAM1 bit 3 (A9, sourced to the annotated
  listing's status-light routine). `tests/port_trace.rs` compares the full
  ordered row stream and the full 4-bit lamp nibble per step **exactly** against
  `rom_port_trace.txt`, 42/42, with no relaxations; mutation checks confirm it
  catches a removed echo row, a swapped symbol column and a wrong lamp bit. See
  `failure_notes/2026-10-18-busicom_141pf-partial-tape.md`.

## Known gaps

- The board model advances one drum sector per `test_pin()` sample instead of
  modelling the two-half-spin-per-sector analog timing (spec ledger M3-P1); the
  reconstructed row is identical to the real-ROM trace, and cycle-accurate
  analog hammer timing is a non-goal (board spec §5.1/§6).
- No target image: the `llvm-i4004` backend (M4) and the on-RTL run (M5) are out
  of scope for M3. `run_all.sh` correctly skips those stages.
- Assumptions A1–A4 / M2-D1…M2-O1 / M3-H1…M3-P1 are validated against the
  recovered-ROM transcripts and the port-level ROM trace; A6–A9 are now
  reproduced and asserted exactly by `tests/port_trace.rs` (each ledger row
  names its source). The one residual unknown is the full meaning of
  `WR.S0.bit1` (RAM1 bit 3): the observable bit is reproduced as
  `overflow || negative`, recorded as an ASSUMPTION in spec A9 and
  `driver_lamps.rs`.
