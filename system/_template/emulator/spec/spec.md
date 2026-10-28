# [SYSTEM] Rust system emulator

Host library and `[SYSTEM]-emulator` executable that run original and compiled
machine-code images through the CPU, memory and peripherals of the whole machine.
This is separate from the title reimplementations and the `llvm-[CHIP]` backend.

## Sources

- TODO (hardware manuals, schematics, `platform/memory_map.md`, `platform/io_map.md`)

## Chip cores and dependencies

| Component | Source (path, commit pin, licence) | Shared with another adapter? |
|---|---|---|
| CPU ([CHIP]) | TODO | TODO |

An emulator that shares a chip core with a reference adapter is not an
independent oracle. State here which cores are shared.

## Machine contract

- Memory map, ROM/RAM sizes, accepted image formats and sizes:
- Peripherals implemented (keyboard/input, display, printer, storage, ...):
- Reset behavior:
- Timing model (cycle-exact, per-instruction, or event-paced) and its limits:

## Input and output contract

- Input format (schedule or transcript) and validation errors:
- Output format and how it maps to the oracle transcripts:
- Exit status for invalid input:

## Unsupported hardware and variants

TODO: list every board variant, peripheral and timing behavior not implemented.

## Assumption ledger

| ID | Assumption | Why the docs don't settle it | How it is validated |
|---|---|---|---|
