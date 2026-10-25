# ibm_pc Rust system emulator

Host library and `ibm_pc-emulator` executable that run original and compiled
machine-code images through the CPU, memory and peripherals of the whole machine.
This is separate from the title reimplementations and the `llvm-i8086` backend.

Status (2026-10-25): the **8088 CPU core** (`src/cpu.rs`) and the **memory and I/O
decode** (`src/machine.rs`) are implemented and verified; the peripherals
(8259, 8253, 8255, 8237, 8272, CGA), the keyboard and diskette paths, and the
replay/CLI contract are not. `src/lib.rs` still carries the template's
placeholder `Emulator`/`replay` until those exist; the CLI is therefore still
the template echo.

## Sources

- Intel, *The 8086 Family User's Manual* (October 1979, 9800722-03), chapters 2 and 4.
- IBM Personal Computer Technical Reference, August 1981 (see `../README.md`).
- `$AI_HW_ENGINEER/system/ibm_pc/spec/spec.md`: the hardware boundary this emulator mirrors.
- `../platform/memory_map.md`, `../platform/io_map.md`.
- Physical-chip instruction vectors: [SingleStepTests/8086](https://github.com/SingleStepTests/8086)
  at commit `e71c68d215a6bb8c356bd4cb3842de3bef345ca9` (Daniel Balsom; generated with
  ArduinoX86 on an Intel P80C86A-2; MIT licence). Not stored here.

## Chip cores and dependencies

| Component | Source (path, commit pin, licence) | Shared with another adapter? |
|---|---|---|
| CPU (8088) | `src/cpu.rs`, written in this crate from the Intel manual and the vectors above | no |
| Memory/I/O decode | `src/machine.rs` | no |
| 8259A, 8253, 8255, 8237A, 8272 | planned: written in this crate | no |
| CGA 6845 | planned: written in this crate | no |

Decision E1: write the 8088 core and peripherals here instead of depending on
`external/martypc`. MartyPC is the secondary oracle, and an emulator sharing its
core would not be an independent check. The oracles are:

- the **physical chip**, through the vectors above (a ground truth neither the
  RTL nor this core derives from);
- the **RTL** in `$AI_HW_ENGINEER` (`design/intel_8086`, and later `system/ibm_pc`),
  which is also validated against those vectors;
- **MartyPC**, read as the reference for behavior the vectors do not cover (E-A5).

No implementation shares code with another, so a disagreement between them is a finding.

## CPU core contract (`src/cpu.rs`)

- Real-mode 8086/8088 integer instruction set, all documented encodings, plus the
  encodings the vector corpus classifies as aliases or undocumented (60-6F as
  70-7F, 82 as 80, C0/C1/C8/C9 as returns, F1 as LOCK, D6 SALC, shift/rotate /6,
  TEST /1, `FF /7`, `0F` as POP CS). `8C`/`8E` with a segment field above 3 use
  the field modulo 4, `8E` into CS loads CS.
- One call to `Cpu::step` runs one instruction with all its prefixes, or one
  interrupt entry. A `REP` string instruction runs all iterations unless an
  interrupt is pending after an element; it then returns to the prefix
  (`IP` = opcode address - 2, so an earlier segment-override prefix is lost, as
  on the chip).
- Memory and I/O go through the `Bus` trait one byte at a time, as the 8088's
  bus sees them: words are low byte then high byte, a word at offset FFFFh wraps
  to offset 0 of the same segment, physical addresses wrap at 1 MiB, a 16-bit
  `IN`/`OUT` is two byte accesses at `port` and `port+1`. Instruction bytes use
  `Bus::fetch`.
- Interrupts: NMI (edge) over INTR (level, needs IF) over the single-step trap,
  all at instruction boundaries; entry reads the vector before pushing FLAGS, CS,
  IP, then clears IF and TF. Divide errors save the address of the next
  instruction. HLT halts until an interrupt.
- Chip quirks reproduced (all exercised by the vector corpus): `PUSH SP` pushes
  the decremented value, shift counts use all eight bits of CL, `IDIV` rejects a
  quotient of -128/-32768, `REP` before `IDIV` negates the quotient, `AAM 0`
  sets the flags for a zero result before trapping, and DAA/DAS follow the chip
  rather than the manual's pseudo-code (E-V1).
- FLAGS bit 1 and bits 12-15 read as one.

Not modeled: the prefetch queue, execution-unit clock counts, bus-cycle timing,
`LOCK`/`HOLD`/`RQ/GT` arbitration, the coprocessor (ESC does a dummy memory read),
and anything the physical-chip corpus calls undefined beyond the encodings above.

## Machine contract (`src/machine.rs`)

- Memory map, ROM/RAM sizes, accepted image formats and sizes: the map in
  `../platform/memory_map.md`. RAM from 00000h (1..640 KiB; the 5150 board
  configuration is 64 KiB), CGA memory B8000h..BBFFFh, ROM F6000h..FFFFFh with CPU
  writes ignored, everything else reads FFh. ROM is loaded top-aligned
  (`load_rom_top`): an 8 KiB BIOS fills FE000h..FFFFFh.
- Peripherals implemented: none yet. Every I/O port reads FFh and writes are
  logged. `Machine::raise_nmi` / `raise_intr(vector)` stand in for the interrupt
  controller until the 8259 model exists.
- Reset behavior: CS:IP = FFFF:0000, FLAGS = F002h, all other registers zero (the
  RTL makes the same choice); RAM is zero-filled (E-A1).
- Timing model: instruction-paced (E-A2); peripheral time will derive from a
  count of executed instructions scaled to the nominal 4.772727 MHz clock, with
  the same limits as the reference platform.

## Input and output contract

Not yet defined (the replay/CLI layer arrives with the peripherals). It will use
the oracle transcripts under `../oracle/transcripts/`, with keyboard input as PC
make/break scan codes stamped with simulated time, and observable output as CGA
text-mode screen contents at checkpoints. Invalid input will fail with its line
number and a nonzero exit status.

## Unsupported hardware and variants

More than 64 KiB of RAM on the 5150 board model, MDA, EGA and later video, hard
disks, the 5160 XT variant, serial/printer/game adapters, cassette I/O,
composite colour artifacts, sub-instruction timing, and undocumented NMOS
behavior beyond the vector corpus.

## Assumption ledger

| ID | Assumption | Why the docs don't settle it | How it is validated |
|---|---|---|---|
| E-A1 | RAM powers up zero-filled | the 5150 RAM contents at power-on are undefined; the BIOS memory test writes before reading | oracle boot of the original BIOS reaches the same screen with zero fill (peripheral milestone) |
| E-A2 | The CPU is modeled per instruction: no prefetch queue, no clock counts. Code written one to four bytes ahead of `IP` is seen immediately, where the chip's queue would still hold the old bytes | the manual describes the queue but not a cycle-exact contract; the reference RTL makes the same simplification | silicon vectors compare state and bus writes, not timing; no title or BIOS routine depends on the stale bytes (to be checked when the BIOS boots) |
| E-A3 | The interrupt shadow (one instruction with interrupts and traps deferred) follows STI, MOV/POP of a segment register, and IRET | Intel documents it for segment loads and STI; for IRET the hardware project's spec says so and the manual is silent | not covered by single-instruction vectors; `tests/cpu_programs.rs` checks the STI case; IRET stays an assumption until a silicon trace or MartyPC cross-check |
| E-A4 | WAIT never waits (the TEST pin is not modeled) | no coprocessor on the 5150 board model | none needed until an 8087 is modeled |
| E-A5 | A REP/REPNE prefix on MUL or IMUL negates the product (as it does the IDIV quotient) | undocumented; the vector corpus has no REP-prefixed MUL/IMUL | MartyPC's microcode-level core does the same (`external/martypc`, `cpu_808x/execute_fn.rs`); the RTL does not, see `report/verification.md` and the failure note of 2026-10-25 |
| E-A6 | The values of flags the manual calls undefined are arbitrary here (the core computes one plausible value) | the manual does not define them | tests mask them with the corpus's `flags-mask`; a program that reads them is outside the contract |
| E-V1 | (validated finding, not an assumption) DAA/DAS: the high-digit adjustment tests the original AL against 9Fh and, for 9Ah..9Fh, only when AF was clear; the low-digit adjustment does not set CF. This differs from the manual's pseudo-code | the manual's pseudo-code reads differently | 4,000 DAA/DAS vectors from the physical chip, all pass; the RTL passes them too |
