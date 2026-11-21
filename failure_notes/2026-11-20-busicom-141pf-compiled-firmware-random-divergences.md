# The compiled firmware's drum sync missed a TEST edge, printing three random-corpus rows one drum sector late

- Area: system/busicom_141pf/firmware (`asm/drivers.s`, `__pr_print`), found
  while running the compiled image on the Rust system emulator
- Found by: replaying `oracle/random_calculator_tests.json` on the compiled
  `firmware.bin` (`--ram 16`) with `platform/scripts/compare_oracle.py`
  (issue #23, 2026-11-20). The 42 manual examples passed 42/42 on the same
  image; 997 of 1000 random programs passed.

## What broke

Three programs printed one row each with every character one drum sector
late: R0534 step 4 (`3115.344300 *` printed as `4226.455411` with the next
column-B glyph), R0568 step 9 (`5.76 =` as `6.87` with the square-root
glyph) and R0872 step 5 (`9621 /` as `.732 M+`). Each digit is +1 (`9` wraps
to `.`), and each symbol is the next glyph on the drum. The register values
were correct: bank-0 RAM after each key matched the host core exactly (a
host-vs-emulator RAM diff per key, `examples/ram_trace.rs` on both sides), so
the defect was in printing, not arithmetic.

## Root cause - and why our tests missed it

A one-cycle race in the drum synchronisation of `__pr_print`. The driver
waited for the drum index (ROM2 bit 0, high for two half-spins), then called
`.Lpr_edge`, which waits for the TEST level to change from the level it sees
on entry. If the index read lands in the last machine cycle of the second
index half-spin, the TEST edge occurs between the index read and that level
sample. `.Lpr_edge` then samples the new level and waits for the following
edge: sector 0 starts one half-spin late, and the whole row fires one sector
late (the sector-0 hammer fires after the sector-12 one, on the next
revolution).

The emulator debugger located it: `hammer_trace` logged every hammer fire
with the drum position and PC. The correct row fired sector 0 at half-spin 2;
the failing row's first fire was at half-spin 4 (`drum=1`), and stepping the
index loop showed `RDR` (opcode `EA`) executing at phase 1480 of half-spin 2
with `index=1`, the next instruction already in half-spin 3.

The window is about one cycle in 1481 and depends on how long each key took
to process, so it appears only for a few of the 4438 keys. The 42 manual
examples never landed in it. The host core has no drum, and the original
ROM's different instruction timing keeps it out of the window for these
programs; the original ROM's own loop has no guarantee either.

## Fix and re-verification

`asm/drivers.s`: the index wait now calls `.Lpr_edge` first and reads the
index just after each edge (the board changes TEST and the index in the same
half-spin step), then calls `.Lpr_edge` once more to start sector 0. The edge
counted can no longer already be behind the driver. Two bytes were recovered
elsewhere to keep the image at 4096 bytes: `.Lpr_edge` returns ACC = 0, so
`xch r6` clears the sector counter inside the loop, and a redundant `clc` in
`.Lpr_bit` is gone (4 + R7 <= 6 leaves carry clear).

Re-verified on the rebuilt image (4096 bytes, SHA-256
`47772d01067bbcae341189b32f8d5f92ee2b574730aaf03a8567e5ecf0baa2fd`):

- the three programs now print exactly what the original ROM prints;
- all 42 manual examples: Rust emulator (`--ram 16`) and the separate
  `chips_trace` adapter both equal the original-ROM oracle;
- random corpus: 1000/1000 (4438 steps, 5325 rows, 1244 lamp checkpoints).

Not run: the RTL primary executor; a drum-phase sweep (below).

## Prevention

- `emulator/tests/replay.rs::compiled_rust_firmware_replays_all_manual_examples`
  runs the compiled image on the emulator (opt-in, `BUSICOM_FIRMWARE`).
- The random corpus replay of the compiled image should become a gate of
  `toolchain/verify.sh`.
- A race this narrow is found by chance, not by design: a test that starts a
  print at every machine cycle of a drum revolution would find it
  deterministically and is the right gate for the printer driver.
- Debugging aids kept in the tree: `emulator/examples/ram_trace.rs` and
  `firmware/examples/ram_trace.rs` (per-key RAM diff, compiled vs host),
  `emulator/examples/hammer_trace.rs` (hammer and paper events with drum
  position, optional per-instruction window).
