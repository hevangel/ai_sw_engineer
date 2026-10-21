# Busicom 141-PF firmware — report

Status: **M5 wrap-up (2026-10-21)** — see the last section. M3/M4 milestone
records below.

## M5 wrap-up (2026-10-21) — host firmware final; target run signed off as out of architectural reach

- **Host behaviour stays 42/42 exact** (recovered-ROM replay, port-level
  replay, and the full-tape `port_trace` all pass; `cargo fmt`/`clippy -D
  warnings` clean, re-verified at wrap-up). All 42 historical manual examples
  replay byte-for-byte against the original-ROM port trace.
- The whole firmware compiles through the `llvm-i4004` backend (43/43
  functions), links banked (BANKS=48, 100,352 B), and executes end-to-end on
  the `chips` simulator — but prints **0/42**. Final root cause: the
  `external/chips` 4002 model latches SRC addresses ignoring the CM-RAM (DCL)
  bank line, so the DCL-banked spill frames clobber the software-return-stack
  chip in bank 0 (see `llvm-i4004/report/final_report.md` STATUS 2026-10-21 and
  `failure_notes/2026-10-19-chips-4002-src-latch-ignores-dcl.md`).
- Underlying limit: the compiled image is ~48 KB, ~11× the 4004's 4,096-byte
  ROM ceiling (`failure_notes/2026-10-19-busicom-firmware-too-big-for-4004.md`);
  the stock 2×4002 board's 64 B of RAM also cannot hold the packed 92 B
  persistent state (M4 fit analysis). Per the 2026-10-21 scope sign-off
  (Option B), the on-target 42-example run is closed as blocked by target
  architecture and simulator fidelity; the verified deliverables above stand.

---

## Milestone M3 record

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

---

## Milestone M4 — target build path, fit analysis, codegen work list (2026-10-19)

This milestone builds the path from the Rust firmware to one optimised
i4004-compatible LLVM IR module, measures whether that firmware fits the
Busicom board, and turns the backend's gaps into a ranked work list. No target
image yet — the `llvm-i4004` codegen backend selects only a fraction of the IR
(see the work list). **Every host test stays 42/42** (the target Hal is
`target_os="none"`-only; the host build is byte-for-byte unaffected).

### Target Hal

`src/main.rs`'s placeholder `BoardHal` is replaced by one that lowers each
`Hal` trait method to a single `__bus_*` C-ABI call into the platform runtime
(`__bus_rom_port_write/read`, `__bus_ram_port_write`, `__bus_test_pin`). These
are the only external symbols left in the firmware IR; the runtime agent
provides them plus `crt0` and `libi4004rt`.

### IR production path

`avr-none` + `-Zbuild-std=core` + fat LTO → one whole-program `.ll`, then a
textual retarget of the header to i4004 (`llvm-i4004/rust/README.md` has the
full mapping and the refusal guards). Output: `build/firmware_i4004.ll`.

| IR metric (whole program, opt-level=z, fat LTO) | Value |
|---|---|
| Defined functions | 40 |
| IR instruction lines (rough) | ~2,400 |
| `float`/`double`/`i128`/`core::fmt`/panic strings | **none** (verified) |
| `i32` uses | 43 |
| `i64` uses | 68 (mostly `llvm.memset` size args + an 8-byte buffer init; packing shifts a few byte copies) |

### Fit analysis — the main risk, and it does NOT fit the stock board

Measured from the IR (`scripts/fit_analysis.py`) and confirmed with host
`size_of`:

| Object | Bytes (1 digit / `u8`) |
|---|---|
| `Num` (`[u8;16]` + `neg` + `scale`) | 18 |
| **`Calculator`** (6 × `Num` + 5 flag/enum bytes) | **116** |
| `KeyBuffer` (8 × `Option<Key>` + head/len) | 24 |
| `Row` | 19 |
| mul/div/sqrt scratch (`[u8; WIDE=34]`) | 34 each, transient |

**Stock Busicom board RAM** (`platform/memory_map.md`): 2 × 4002 = **128 main
nibbles = 64 bytes** (+ 32 status nibbles + 16 CPU index registers).

- The **persistent** state alone — `Calculator` (116) + `KeyBuffer` (24) = **140
  bytes** — is **2.2× the 64-byte main memory**. Peak simultaneous RAM, adding a
  live `Row` and a `WIDE` scratch buffer during a multiply/divide, is ~210
  bytes.
- **Verdict: the firmware as represented does not fit the stock board**, and no
  amount of flag bit-packing closes a 2× gap on its own.

Why: the current `Num` stores **one BCD digit per byte**, but the 4002 stores
one digit per **nibble**. LLVM's minimum addressable unit is a byte, so the
natural Rust representation wastes exactly 2× against the hardware the original
ROM actually used (the original kept its ~7 working registers directly in the
4002 nibble files).

### What fixes it, and the honest limit

1. **Pack two BCD digits per byte** — **DONE in this milestone.** `Num.d` is now
   a private `[u8; 8]` (two nibbles per byte) behind `digit(i)` / `set_digit(i,v)`
   accessors; every call site (`bcd.rs` arithmetic, `lib.rs`, `print.rs`, the
   tests) goes through them, and the digit-serial algorithms are byte-for-byte
   the same on the same digit values. **Measured** result (`size_of`, host):
   `Num` 18 → **10 bytes**, `Calculator` 116 → **68 bytes**. The transient
   mul/div/sqrt `[u8; WIDE=34]` scratch is deliberately left one-digit-per-byte
   (it is not resident between keystrokes, so it does not affect the persistent-
   RAM verdict). All host tests stay **42/42** (incl. the exact tape); `fmt` and
   `clippy -D warnings` clean.
2. **Even fully packed, the persistent register file is 68 B (`Calculator`)
   + 24 B (`KeyBuffer`) = 92 B > 64 B.** So packing is *necessary but not
   sufficient* for the stock board. To fit the stock 2×4002 board the firmware
   would additionally have to stop holding all six registers live at once as one
   struct and instead allocate them into the 4002 nibble files with the reuse
   the original ROM used (WR/DR/CR/SR/TR/MR were not all simultaneously
   resident), plus move `KeyBuffer` into status characters / index registers.
3. **Minimal board that fits without that re-architecture:** the packed
   `Calculator` (68 B) + `KeyBuffer` (24 B) + transient scratch (~20 B) needs on
   the order of **~112 bytes of main RAM**, i.e. **4 × 4002** (256 nibbles =
   128 bytes) rather than the stock 2. ROM: the 4004 architecture allows up to
   16 × 4001 = **4096 bytes**; the generated image size against both the 1280-
   byte original budget and the 4096-byte ceiling is **pending the codegen
   backend** (llc cannot yet lower the firmware).

### RAM: before and after the packing

| Object | Before (1 digit/byte) | After (2 digits/byte) |
|---|---|---|
| `Num` | 18 B | **10 B** |
| `Calculator` (6 × `Num` + flags) | 116 B | **68 B** |
| biggest single IR object (`dereferenceable`) | 116 B | **68 B** |
| `KeyBuffer` | 24 B | 24 B (unchanged) |
| persistent register file (`Calculator`+`KeyBuffer`) | 140 B | **92 B** |

Stock board main memory is **64 B** throughout: the firmware does not fit the
stock board before OR after packing; packing takes the gap from 2.2× to 1.4×
and halves the biggest object, and the stock-board fit needs the register re-
architecture in item 2 (or the 4×4002 board in item 3).

### Representation packing — applied

The packing was a ~40-site change inside `src/bcd.rs` (every `d[i]` digit
access in `cmp_mag`, `shl`, `shr`, `inc_mag`, `add_mag`, `sub_mag`, `mul`,
`divide`, `sqrt_at`, `from_mag`), plus the test helpers (`num`, `render`, a new
`low_digits`) and the `pub` field uses in `lib.rs` (`wr.set_digit(0, …)`) and
`print.rs` (`n.digit(i)`). `Num.d` is now **private**, so no code can touch the
byte layout without the accessors. The change is behaviour-preserving (the
algorithms run on the same digit values) and is validated by the full host
suite staying **42/42**, including the exact-tape `port_trace` — the strongest
possible regression gate for a representation change, since it compares the
entire printed tape and lamp stream against the original ROM. `fmt` and
`clippy -D warnings` are clean. The "after" sizes are **measured** (host
`size_of`), not estimates.

### Codegen work list (`llvm-i4004/rust/codegen_worklist.md`)

`llc` aborts on the first unselectable node, so `scripts/gen_worklist.py` splits
the module into one function per run and aggregates. On the **packed** firmware
IR: **2 / 43 functions select**, the rest blocked. Top blockers:

| Rank | Failure bucket (first blocking node) | Functions |
|---|---|---|
| 1 | `assert:*` — incomplete DAG legalisation (SELECT/SELECT_CC, …) | 12 |
| 2 | `llvm-error` — `Cannot select` on an unmapped node | 11 |
| 3 | AsmPrinter crash (missing instruction mnemonic) | 7 |
| 4 | `load` | 5 |
| 5 | `store` | 4 |

Each function reports only its *first* blocking node, so the list is a
burn-down: re-run after each codegen landing to reveal the next layer. This is
the backlog the codegen agent needs. Packing shifted the profile from the
unpacked firmware's `zero_extend` (11) + i16 `Constant` (9) dominance — digit
addressing no longer widens an `i8` index to `i16` per access — toward the
nibble shift/mask `load`/`store` traffic and the still-missing `SELECT`
legalisation and AsmPrinter mnemonics. The generated table in
`codegen_worklist.md` has the per-operation detail and the function lists, and
is regenerated by `gen_worklist.py` on each rebuild.
