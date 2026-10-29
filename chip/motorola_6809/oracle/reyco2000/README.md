# Oracle — reyco2000/6809_6309_code_tester

The validation fixtures for the MC6809 core: the unmodified test programs
of https://github.com/reyco2000/6809_6309_code_tester, **GPL-3.0-or-later**
(`license.txt` here is the upstream copy; the fetched `.asm`/`.inc` files
carry the same header). Fetched 2026-10-26 from the upstream `src/`
directory.

- `6809_functional_test.s19` / `.asm` / `.lst` — every documented
  instruction and addressing mode; self-checking (a failing check
  branches to itself; look the PC up in the `.lst`).
- `6809_interrupt_test.s19` / `.asm` / `.lst` — IRQ/FIRQ/NMI semantics,
  masking, frames, CWAI and SYNC; needs the DEV test device.
- `6809_cycle_test.s19` / `.asm` — data-sheet cycle counts per
  instruction form via the DEV cycle-counter latches; reports every
  mismatch to the DEV console; exit code = mismatch count.
- `common.inc`, `test_macros.inc` — shared definitions and macros used
  by the sources.

The `.s19` images are what the tests load; the Rust harness
(`emulator/tests/common/mod.rs`) ports the suite's DEV device
($FF00-$FF0F) exactly as the ai_hw_engineer RTL testbench implements it.
