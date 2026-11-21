# Busicom Rust system emulator

The host library and `busicom-emulator` executable run machine code on the
pinned, unmodified `external/chips` MCS-4 models. The system belongs here; CPU,
4001, 4002 and 4003 implementations remain in that dependency. This shares its
core with `platform/scripts/chips_trace.rs`, so agreement between them is not
independent validation. The original-ROM trace in
`../../firmware/oracle/rom_port_trace.txt` records output previously checked
against the hardware project's RTL (see its oracle README).

## Board and input contract

- Stock wiring: five 4001 ROMs, two 4002 RAMs, three 4003 shifters, keyboard,
  decimal/rounding selectors, drum printer and lamps. Memory and wiring follow
  `../../platform/memory_map.md` and `../../platform/io_map.md`.
- Accept 1–4096 ROM bytes; zero-pad to at least five whole 256-byte ROMs,
  allowing the 4095-byte compiled image without a partial-chip panic. More than
  five ROMs is the documented flat ROM expansion, not the original capacity.
  RAM is two chips by default; `Emulator::with_ram` and `--ram N` accept 2 to
  16. Chip `i` is chip `i % 4` of CM-RAM bank `i / 4`, selected by DCL, as on
  the xl board; chips 0 and 1 keep the printer and lamp roles. ROM
  bank-switching (images over 4096 bytes) is unsupported; use the existing
  platform runner for that variant.
- A reset reconstructs all chips and peripheral state using the loaded image.
- Input is the schedule from `platform/scripts/build_schedule.py`: example
  markers reset, reset/step markers checkpoint, numeric events specify
  `precision rounding key half-spins`. Precision: 0–6 or 8; rounding: 0, 1 or 8;
  key: 0 (released) or 129–160. Each event holds the key for at most 64
  half-spins then releases it. Invalid input fails with its line number.
- One half-spin advances 1481 chip machine cycles. Drum index wraps at 26;
  TEST uses the pinned core's compensated polarity, exactly as documented in
  the platform README. This is deterministic reference pacing, not a physical
  motor simulation or transistor-level timing model.

## Debug contract

`busicom_emulator::debug` steps whole instructions (a two-cycle JCN, FIM, FIN,
JUN, JMS or ISZ is one step). Each step reports the CPU state before the
instruction (PC, ACC, carry, TEST, R0..RF), the opcode byte, the second byte
where one exists, and any printer rows emitted. State is captured from the
pinned model's `log::debug!` record at each instruction boundary because its
registers are private and `external/` is not edited. Breakpoints stop after
the instruction at the address executed. RAM characters, status nibbles and
the 4002/4001 port nibbles are readable at any time. Stepping uses the same
cycle, drum and I/O path as `advance`, so a stepped run and a replay are
cycle-identical. Not provided: watchpoints on writes, disassembly, reverse
stepping and mid-instruction state.

## Output contract

Emit `#EXAMPLE`, `#RESET`, `#STEP`, `LAMPS|n` and
`ROW|value|symbolA|symbolB|red` in the existing oracle format. A rising paper
advance flushes a nonblank row; hammer edges select drum glyphs using the
20-bit chain; red latches until advance. Step checkpoints expose the full RAM1
output nibble. No ROM binary is embedded or committed. ROM provenance remains
in `../../firmware/original/provenance.md`.
