# Atari 400/800 emulator — verification report (first slice)

- Date: 2026-10-27
- Agent: GLM (ZCode)
- Branch: `system/atari_400_800` (ai_sw_engineer), working tree at this commit
- Issue: [ai_sw_engineer#170](https://github.com/hevangel/ai_sw_engineer/issues/170)
- Environment: Windows 10.0.26300 x64, rustc/cargo 1.99.1 stable
  (`stable-x86_64-pc-windows-gnu`), rustfmt + clippy components installed

## Dependency provenance

| Dependency | Pin | License |
|---|---|---|
| `external/mos6502` (6502-rs) | submodule `ffcc9bc` | BSD-3-Clause |
| `external/6502_65C02_functional_tests` (Klaus2m5) | submodule `7954e2d` | see `license.txt` in the submodule |

Both used as pinned submodules; neither was modified.

## Commands and results

Run from `system/atari_400_800/emulator/`:

```
$ cargo test
running 1 test
test klaus_dormann_functional_test_passes ... ok        [~7.5 s]
running 10 tests
test gtia_pia_and_hardware_mirror_decode ... ok
test identical_runs_are_bit_identical ... ok
test pokey_irqenable_marks_pending_and_vectors ... ok
test pokey_random_is_deterministic_across_machines ... ok
test profiles_cart_and_open_bus ... ok
test reset_fetches_os_rom_reset_vector ... ok
test vbi_nmi_fires_once_per_nmires_and_status_tracks_it ... ok
test vbi_status_sets_at_line_248 ... ok
test vcount_reads_scanline_divided_by_two ... ok
test wsync_halts_cpu_until_end_of_scanline ... ok
test result: ok. 11 passed; 0 failed (across all targets)
```

```
$ cargo fmt --check          # clean (after cargo fmt)
$ cargo clippy --all-targets -- -D warnings
    Finished `dev` profile ... in 4.72s   # zero warnings
```

CLI smoke test (full Klaus Dormann run through the executable, not just the
test harness):

```
$ cargo build --release
$ ./target/release/atari-400-800-emulator.exe --profile flat64k \
    --image ../../../external/6502_65C02_functional_tests/bin_files/6502_functional_test.bin@0 \
    --pc 0400 --cycles 200000000 --dump-state
loaded 65536 bytes from .../6502_functional_test.bin at $0000
PC=$3469 A=$F0 X=$0E Y=$FF S=$FF P=$F1
cpu_cycles=200000075 virtual_cycles=200000075 frames=6662 scanline=205
```

`PC=$3469` is the suite's "test passed, no errors" trap: every documented
NMOS 6502 opcode and addressing mode, in binary and decimal mode, executed
through this emulator's bus decode and reached the success trap, and the
run is deterministic (the harness asserts a second identical run lands on
the same trap after the same number of virtual cycles).

## What this verifies

- CPU semantics (via the pinned core) against an independent artifact
  (Klaus Dormann's suite) through this emulator's own bus fetches.
- Address decode and hardware mirror spacing (GTIA 32 B / POKEY 16 B /
  PIA 4 B / ANTIC 16 B), 16K/48K RAM profiles, cartridge overlay and write
  protection, open-bus regions.
- Machine timing skeleton: 114 cycles/scanline × 262 lines, WSYNC halt,
  VCOUNT, NMIST at line 248, vertical-blank NMI with the NMIRES re-arm
  edge, POKEY IRQEN marks-pending model with delayed-I sampling, POKEY
  RANDOM determinism, PIA 6520 DDR semantics.
- Whole-run determinism.

## What this does NOT verify (honest gaps)

- No independent whole-machine oracle has been run yet (no third-party
  Atari 8-bit emulator pinned; the `ai_hw_engineer` machine RTL does not
  exist yet). The CPU-level RTL cross-check against
  `$AI_HW_ENGINEER/design/mos_6502` is planned, not run. See
  [../spec/spec.md](../spec/spec.md#oracle-plan) and
  [../plans/testplan.md](../plans/testplan.md).
- ANTIC display-list execution, DMA/cycle stealing, video output, POKEY
  audio, serial/SIO/disk, keyboard/joystick input and PAL timing are not
  implemented (spec scope gaps A8-K1..A8-K10, TA-1..TA-3).

## Escaped defects found and fixed during this slice

- `Machine::step_scanline` did not reset the per-line cycle budget, so the
  first scanline after reset consumed the whole run (found via a
  per-instruction probe against the core; Klaus passes 30.6M instructions /
  96.2M cycles in ~8 s once fixed).
