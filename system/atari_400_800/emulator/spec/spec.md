# Atari 400/800 emulator — machine contract and assumption ledger

Status: **first working slice** (see [../README.md](../README.md) for what
runs and [../../plans](../../plans) — this file is the machine contract;
everything it claims is enforced or explicitly listed as a gap).

Tracked as [ai_sw_engineer#170](https://github.com/hevangel/ai_sw_engineer/issues/170);
the machine contract is shared with the hardware/SV system issue
[ai_engineer#355](https://github.com/hevangel/ai_engineer/issues/355), which
does not require its RTL to exist for this contract to be valid.

## Machine identity

- Historical machine: Atari 400 and Atari 800, NTSC, stock RAM
  configurations (16 KiB / 48 KiB).
- CPU: Atari CO14806 "SALLY" (marketed as 6502C) — an NMOS 6502 core with a
  HALT line. **Not** a WDC 65C02. Modeled by the pinned NMOS 6502 core
  (`mos6502` crate, `Nmos6502` variant) with decimal mode enabled
  (assumption A8-K1).
- Clock: PHI2 = 1.7897725 MHz (NTSC); 114 CPU cycles per scanline, 262
  scanlines per frame; vertical-blank NMI at the start of scanline 248
  (assumption A8-K2). PAL is not implemented.
- Profiles (`MachineProfile`): `Atari400`, `Atari800`, and `Flat64K` — a
  bare-6502 test bus (64 KiB RAM, no hardware decode, no interrupts) that
  exists to run CPU functional suites, **not** a historical machine.

## Dependencies and provenance

| Dependency | Pin | License | Role |
|---|---|---|---|
| `external/mos6502` (6502-rs) | submodule `ffcc9bc` | BSD-3-Clause | CPU core (NMOS 6502) |
| `external/6502_65C02_functional_tests` (Klaus Dormann) | submodule `7954e2d` | see `license.txt` in the submodule | CPU functional test binary |

No other runtime dependencies. The emulator never edits `external/` in
place. Recorded per ai_sw_engineer/AGENTS.md ("record dependencies and
unsupported hardware explicitly").

## Machine contract

### Memory and decode

Address map and budgets: [platform/memory_map.md](../../platform/memory_map.md).
Hardware-page register offsets and mirror spacing:
[platform/io_map.md](../../platform/io_map.md). Decode rules:

- Hardware page decodes in four $200-byte slots; GTIA repeats every 32 B,
  POKEY every 16 B, PIA every 4 B, ANTIC every 16 B through the slot.
- Untouched regions read $FF; writes are lost (A8-K3). OS/FP ROM regions
  ship erased ($FF); writes are lost.
- Cartridge: 8 KiB at $A000 or 16 KiB at $8000; writes lost; no bank
  switching (A8-K10).

### CPU-visible timing

- The machine advances in scanlines: up to 114 virtual CPU cycles per line;
  a WSYNC write halts the CPU at the end of the executing instruction until
  the next line (TA-1).
- VCOUNT returns scanline >> 1; NMIST bit 6 sets at the start of line 248;
  the NMI line to the CPU stays asserted while an enabled NMIST source is
  pending, and NMIRES clears it — software sees one NMI per vertical blank
  unless it resets the status (matches the ANTIC NMI hardware model).
- The POKEY poly counter advances once per bus access (TA-2).
- The 8-cycle reset sequence is not charged (TA-3).

### Interrupts

- NMI: ANTIC-sourced (VBI this slice; DLI reserved with the display-list
  work). Edge behavior via the core's NMI line sampling.
- IRQ: POKEY-sourced. IRQEN writes mark newly enabled sources pending
  (active-low IRQST) per the POKEY interrupt system; the line is level
  tested against the core's delayed-I sampling. Keyboard/serial/audio-timer
  sources never fire in this slice (A8-K6).

### Peripherals

Implemented this slice: register decode and storage for all four chips, the
GTIA/POKEY/PIA/ANTIC read maps for program-visible constants (triggers,
console keys, PAL flag, KBCODE, SKSTAT, IRQST, RANDOM, VCOUNT, NMIST), the
POKEY IRQ model, ANTIC WSYNC/VBI, and the PIA 6520 DDR semantics.

Not implemented (explicit gaps): ANTIC display-list execution, DMA and
cycle stealing, any video/framebuffer output; POKEY audio output and
audio-timer IRQs; serial I/O / SIO and disk drives; keyboard and joystick
input; cassette; light pen; PAL timing; cartridge bank switching.

### Determinism

No wall clock, no host entropy, no threads: identical construction and
identical run commands produce bit-identical state. Enforced by
`tests/machine.rs::identical_runs_are_bit_identical` and the second-run
check in `tests/functional_6502.rs`.

## Copyright

No copyrighted ROM, OS, BASIC, DOS or cartridge image is committed or
distributed with this emulator. `--os-rom`, `--cart` and `--image` exist so
the user can load legally obtained images; acquisition and license status
are recorded per title under `original/provenance.md` when software-title
work starts.

## Oracle plan

1. **CPU semantics** (now): Klaus Dormann's 6502 functional suite runs
   through this emulator's bus — an independent artifact, not derived from
   the emulator under test.
2. **CPU vs RTL** (planned): cross-check instruction-level state against the
   NMOS 6502 RTL in `$AI_HW_ENGINEER/design/mos_6502` (primary oracle per
   AGENTS.md). A mismatch is a finding, not noise.
3. **Whole machine** (planned): pin an independent third-party Atari 8-bit
   emulator under `external/` (candidates: `atari800` (GPL), MAME (GPL);
   Altirra is a closed-source reference, not a submodule candidate) and
   replay timed inputs against captured observable outputs. Not pinned yet —
   tracked on the issue.

## Assumption ledger

IDs are cited in code as `ASSUMPTION (spec A8-Kn)` / `(spec TA-n)`; the
I/O-facing entries are also listed in [platform/io_map.md](../../platform/io_map.md).

| ID | Assumption | Validation |
|---|---|---|
| A8-K1 | SALLY = NMOS 6502 core; HALT/DMA halt not modeled (only WSYNC) | title four-way runs once DMA exists; RTL cross-check |
| A8-K2 | NTSC only: 1.7897725 MHz, 262 lines, 114 cycles/line, VBI at line 248 | independent-emulator comparison once pinned |
| A8-K3 | Untouched regions read $FF, writes lost; writes above RAM top lost | OS-level comparisons; revisit if a title depends on floating bus |
| A8-K4 | Write-only registers read $00 | OS boot comparisons |
| A8-K5 | Light pen PENH/PENV read $00 | none until a light-pen title is chosen |
| A8-K6 | IRQST reset $FF; SKSTAT idle $EF (bit 4 = 0); keyboard/serial IRQs never fire | OS keyboard/serial paths once input lands |
| A8-K7 | RANDOM = 17-bit LFSR (reset $1FFFF, taps x^17+x^13, output bits 17..10); sequence approximate until verified | independent-emulator comparison once pinned |
| A8-K8 | KBCODE reset $3F | OS keyboard init comparison |
| A8-K9 | POT0-7/ALLPOT read $FF; PIA ports read $FF; CA2/CB2 not modeled | joystick titles in the four-way matrix |
| A8-K10 | Cart 8K at $A000 / 16K at $8000; writes lost; no bank switching | run a bank-switched cart title; document the gap |
| TA-1 | Instruction-granular scheduling: WSYNC at end of instruction; VBI at start of line 248; no DMA cycle stealing | timing-sensitive title comparison |
| TA-2 | POKEY poly advances once per bus access (≈ PHI2); exact alignment unverified | independent-emulator comparison |
| TA-3 | Reset sequence (8 cycles) not charged to the cycle count | irrelevant to software behavior; revisit only for cycle-exact traces |
