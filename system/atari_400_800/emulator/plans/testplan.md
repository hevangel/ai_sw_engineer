# Atari 400/800 emulator — test plan

Written before implementation (per AGENTS.md, Simulators) and kept current.

## Implemented tests

| Test | Verifies | Evidence |
|---|---|---|
| `tests/functional_6502.rs::klaus_dormann_functional_test_passes` | Every documented NMOS 6502 opcode and addressing mode (binary + decimal) through the emulator's own bus fetches, using the pinned Klaus Dormann suite (full 64K image, PC $0400, success trap $3469); plus run-to-run determinism | `cargo test -p atari-400-800-emulator` |
| `tests/machine.rs::reset_fetches_os_rom_reset_vector` | Reset sequence reads the OS ROM vector; RAM writes through the decode work | same |
| `tests/machine.rs::wsync_halts_cpu_until_end_of_scanline` | WSYNC halts the CPU to end of line; virtual cycles stay on the 262×114 frame grid | same |
| `tests/machine.rs::vcount_reads_scanline_divided_by_two` | VCOUNT decode | same |
| `tests/machine.rs::vbi_status_sets_at_line_248` | NMIST bit 6 at start of line 248 | same |
| `tests/machine.rs::vbi_nmi_fires_once_per_nmires_and_status_tracks_it` | NMI vectoring, one-shot edge until NMIRES, NMIST readback | same |
| `tests/machine.rs::pokey_irqenable_marks_pending_and_vectors` | IRQEN marks pending (active low), IRQ vectoring, delayed-I sampling, handler clears via IRQEN rewrite | same |
| `tests/machine.rs::pokey_random_is_deterministic_across_machines` | RANDOM steps between reads; identical machines produce identical sequences | same |
| `tests/machine.rs::gtia_pia_and_hardware_mirror_decode` | GTIA read map (TRIG/CONSOL/PAL), PIA 6520 DDR semantics, 32/16/4-byte hardware mirrors | same |
| `tests/machine.rs::profiles_cart_and_open_bus` | 16K/48K RAM tops, cartridge overlay and write protection, open-bus regions | same |
| `tests/machine.rs::identical_runs_are_bit_identical` | Determinism across the whole machine state | same |

## Planned tests (in order)

1. **CPU vs RTL cross-check** — drive `$AI_HW_ENGINEER/design/mos_6502` RTL
   and this emulator from the same instruction stream; compare architectural
   state every instruction. Primary-oracle requirement (AGENTS.md,
   Simulators). Needs an SV/ISS runner from the hardware side.
2. **Independent whole-machine oracle** — pin `atari800` or MAME under
   `external/`; replay timed inputs; compare observable outputs. Decision
   tracked on the issue.
3. **Display-list execution** — once implemented: DLI NMI timing per mode
   line, NMIST bit 7, NMIRES, DL walk over a hand-built display list.
4. **POKEY timer IRQs** — AUDF/AUDCTL timer reloads, IRQ sources 1-3, STIMER.
5. **Keyboard input model** — KBCODE/SKSTAT transitions, IRQ source, and a
   deterministic input replay harness.
6. **Four-way matrix replays** — per software title (#139-#142): original ×
   rewrite on open-source emulator × this emulator, same timed inputs, same
   observable outputs (AGENTS.md software title lifecycle step 8).
7. **PAL profile** — 177 lines behavior, PAL register read $1F, 50 Hz timing.
