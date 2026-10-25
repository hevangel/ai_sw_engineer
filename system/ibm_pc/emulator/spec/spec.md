# ibm_pc Rust system emulator

Host library and `ibm_pc-emulator` executable that run original and compiled
machine-code images through the CPU, memory and peripherals of the whole machine.
This is separate from the title reimplementations and the `llvm-i8086` backend.

Status (2026-10-25): the **8088 CPU core** and the **5150 board** (memory and I/O
decode, interrupt controller, timer, PPI and keyboard, DMA, diskette controller,
CGA registers and text screen) are implemented. The unchanged IBM BIOS boots PC
DOS 1.00 from a diskette image and runs typed DOS and BASIC sessions whose screens
and written disk images match the hardware project's RTL (see
`report/verification.md`). Not done: CGA graphics output and a frame renderer,
the speaker, a graphical front end, and running compiled `llvm-i8086` images
(there are none yet).

## Sources

- Intel, *The 8086 Family User's Manual* (October 1979, 9800722-03), chapters 2 and 4.
- IBM Personal Computer Technical Reference, August 1981 (see `../README.md`).
- Datasheets: Intel 8259A, 8253, 8237A, 8255A, 8272 (and NEC uPD765).
- `$AI_HW_ENGINEER/system/ibm_pc/spec/spec.md`: the hardware boundary this emulator mirrors.
- `../platform/memory_map.md`, `../platform/io_map.md`.
- Physical-chip instruction vectors: [SingleStepTests/8086](https://github.com/SingleStepTests/8086)
  at commit `e71c68d215a6bb8c356bd4cb3842de3bef345ca9` (Daniel Balsom; generated with
  ArduinoX86 on an Intel P80C86A-2; MIT licence). Not stored here.
- Original firmware and disk: `../original/provenance.md`.

## Chip cores and dependencies

| Component | Source | Shared with another adapter? |
|---|---|---|
| CPU (8088) | `src/cpu.rs`, written here from the Intel manual and the vectors above | no |
| Board decode, clock | `src/machine.rs` | no |
| 8259A PIC | `src/pic.rs` | no |
| 8253 PIT | `src/pit.rs` | no |
| 8255 PPI, keyboard | `src/ppi.rs` | no |
| 8237A DMA | `src/dma.rs` | no |
| 8272 FDC | `src/fdc.rs` | no |
| CGA 6845 registers, text screen | `src/cga.rs` | no |

Decision E1: write every core here instead of depending on `external/martypc`.
MartyPC is the secondary oracle, and an emulator sharing its code would not be an
independent check. The oracles are:

- the **physical chip**, through the vectors above (ground truth for instructions);
- the **RTL** in `$AI_HW_ENGINEER` (`design/intel_8086` for the CPU, `system/ibm_pc`
  for the whole machine, running the same unchanged firmware and disk);
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

## Board contract (`src/machine.rs` and the device modules)

**Memory** (`../platform/memory_map.md`): RAM from 00000h (1..640 KiB; the 5150 board
configuration is 64 KiB), CGA memory B8000h..BFFFFh (16 KiB aliased twice),
ROM F6000h..FFFFFh with CPU writes ignored, everything else reads FFh. ROM is
loaded top-aligned (`load_firmware`: 8 KiB BIOS at FE000h, optional 32 KiB
Cassette BASIC below it).

**I/O** (`../platform/io_map.md`), decoded on A9:A5 like the planar, so higher
address bits alias:

| Ports | Device | Modeled |
|---|---|---|
| 00h-0Fh | 8237A DMA | register file, flip-flop, mask, mode, status, terminal count, auto-initialize, address decrement; channel 2 transfers for the diskette controller |
| 20h-21h | 8259A PIC | ICW1-ICW4, OCW1-OCW3, edge-triggered, fixed priority, non-specific and specific EOI, IRR/ISR reads |
| 40h-43h | 8253 PIT | modes 0-5, binary/BCD flag, 16-bit and byte access, counter latch, gate on channel 2; channel 0 drives IRQ0, channel 2 reaches port C bits 4-5 |
| 60h-63h | 8255 PPI | mode 0; port A = SW1 (PB7 set) or the keyboard latch, port B output latch (timer-2 gate, keyboard clock and acknowledge), port C input (timer-2 output) |
| 80h-83h | DMA page latches | per IBM wiring: 81h channel 2, 82h channel 3, 83h channel 1 |
| A0h | NMI mask | latched, no NMI source |
| 3D0h-3DFh | CGA | 6845 index/data, mode and colour-select registers, status (retrace bits) |
| 3F2h, 3F4h-3F5h | diskette adapter | DOR (drive select, reset, DMA/IRQ enable, motors), 8272 main status and data |

Everything else reads FFh. Absent adapters (serial, printer, game) are absent.

**Diskette controller** (E-A10): up to four single-sided 160 KiB drives (40 cylinders,
8 sectors of 512 bytes). Commands: SPECIFY, SENSE DRIVE STATUS, SENSE INTERRUPT STATUS,
RECALIBRATE, SEEK, READ DATA, WRITE DATA, READ ID, FORMAT TRACK; anything else returns
ST0 = 80h. The command, execution and result phases, interrupts, and the DMA data path
follow the datasheet; head step, rotation and byte times are fixed. A drive reads and
writes only with a disk inserted and its motor on.

**Time** (E-A2, E-A11): the board clock counts CPU clocks (4.772727 MHz). An instruction
costs 4 clocks per byte moved on the bus plus 2; HLT idles 16 clocks per step. The timer
runs at a quarter of the clock. The CGA frame is 79,545 clocks with vertical retrace in
the last 6,545 and horizontal blanking from clock 240 of each 303-clock line.

**Reset behavior**: CS:IP = FFFF:0000, FLAGS = F002h, other CPU registers zero; RAM and
CGA memory zero-filled (E-A1); devices re-created; ROM and mounted disks kept.

## Input and output contract

A **session script** (`src/session.rs`) boots the machine and types keys through scan
codes (`src/keys.rs`, US layout, shift handled) and records the CGA text screen when it has
settled. Transcript format:

```
boot | type <text>        (\r is Enter)
until contains|endswith|row <text>
screen                    (output) 25 rows, each prefixed '|' and right-trimmed
disk 1 sha256 <hex>       (output) SHA-256 of drive B's image
```

A step ends when the keyboard queue is empty, the screen has changed and the condition
holds, plus 2,000,000 clocks to settle. The RTL oracle (`scripts/rtl_oracle.py`) records
the same format from the hardware project's RTL, so a replay is a text comparison.
`ibm_pc-emulator --bios B [--basic B] [--disk-a I] [--disk-b I] [--ram KiB] session.txt`
prints the transcript; invalid input fails with a line number, a timeout or I/O error
exits nonzero. Drive A is mounted write-protected.

## Unsupported hardware and variants

More than 64 KiB of RAM on the 5150 board model, MDA, EGA and later video, CGA
graphics-mode rendering and light pen, the speaker, hard disks, double-sided and
8-inch diskettes, the 5160 XT variant, serial/printer/game adapters, cassette I/O,
NMI sources (parity, coprocessor), composite colour artifacts, sub-instruction
timing, and undocumented NMOS behavior beyond the vector corpus.

## Assumption ledger

| ID | Assumption | Why the docs don't settle it | How it is validated |
|---|---|---|---|
| E-A1 | RAM and CGA memory power up zero-filled | the 5150 RAM contents at power-on are undefined; the BIOS memory test writes before reading | the original BIOS passes POST and boots DOS with zero fill; the RTL makes the same choice |
| E-A2 | Time is per instruction: no prefetch queue, no clock counts; an instruction costs 4 clocks per bus byte plus 2. Code written one to four bytes ahead of `IP` is seen immediately, where the chip's queue would still hold the old bytes | the manual describes the queue but not a cycle-exact contract; the reference RTL also does not match original EU timing | silicon vectors compare state and bus writes, not timing; the BIOS and DOS timeouts and tick counts work (DOS session matches the RTL); no code relies on the stale bytes |
| E-A3 | The interrupt shadow (one instruction with interrupts and traps deferred) follows STI, MOV/POP of a segment register, and IRET | Intel documents it for segment loads and STI; for IRET the hardware project's spec says so and the manual is silent | not covered by single-instruction vectors; `tests/cpu_programs.rs` checks the STI case; IRET stays an assumption until a silicon trace or MartyPC cross-check |
| E-A4 | WAIT never waits (the TEST pin is not modeled) | no coprocessor on the 5150 board model | none needed until an 8087 is modeled |
| E-A5 | A REP/REPNE prefix on MUL or IMUL negates the product (as it does the IDIV quotient) | undocumented; the vector corpus has no REP-prefixed MUL/IMUL | MartyPC's microcode-level core does the same (`external/martypc`, `cpu_808x/execute_fn.rs`); the RTL does not, see `report/verification.md` and the failure note of 2026-10-25 |
| E-A6 | The values of flags the manual calls undefined are arbitrary here (the core computes one plausible value) | the manual does not define them | tests mask them with the corpus's `flags-mask`; a program that reads them is outside the contract |
| E-A8 | The keyboard is modelled at the completed-scan-code boundary: holding PB6 low for 100,000 clocks (about 21 ms) and releasing it makes the keyboard answer AAh after 1,000 clocks; PB7 high clears the latch and IRQ1; queued codes are delivered one at a time when the latch is free | the serial protocol and the keyboard's microcontroller are outside the board; the thresholds mirror the hardware project's assumption A4 | the BIOS keyboard test and DOS typing work; the DOS session matches the RTL |
| E-A9 | DRAM refresh (timer channel 1, DMA channel 0) is not simulated: RAM is static and the CPU is not stalled; channel 0's registers still read and write | the refresh does not change what programs see; the hardware project makes the same choice (its A3) | the BIOS DMA register test passes; no program observes the refresh |
| E-A10 | The diskette controller works at command level with fixed times: 6 ms per cylinder step, 200 ms per revolution (sector time 1/8 of it), 32 microseconds per byte; SPECIFY is ignored; READY is always true (the 5150 adapter has no READY input) but a drive transfers data only with a disk inserted and the motor on; after reset release one polling interrupt (ST0 = C0h) is raised, and the first SENSE INTERRUPT STATUS clears all four | the datasheet gives timing as programmable intervals and polling as four interrupts; the BIOS is written for the 5150 adapter | the BIOS POST disk test, boot, DOS reads and writes, FORMAT and COPY all work, and drive B's image after FORMAT and COPY is byte-identical to the RTL's (transcript `disk` lines) |
| E-A11 | The CGA frame is 79,545 CPU clocks (60 Hz) with vertical retrace in clocks 73,000 and up, and horizontal blanking from clock 240 of each 303-clock line | the Technical Reference gives the dot clock and line counts; the retrace bit pattern a BIOS polls is what matters | the BIOS video routines, which poll the status port, run; the hardware project uses the same figures |
| E-A12 | The planar switches read SW1 = 6Fh (two drives, 80x25 colour, no 8087, 64 KiB) and SW2 = 0 | the board's DIP switches are user settings; these are the values the hardware project's board presents, so the same firmware sees the same machine | BASIC reports 61,404 bytes free and CHKDSK reports 65,536 bytes of memory, as on the RTL |
| E-V1 | (validated finding, not an assumption) DAA/DAS: the high-digit adjustment tests the original AL against 9Fh and, for 9Ah..9Fh, only when AF was clear; the low-digit adjustment does not set CF. This differs from the manual's pseudo-code | the manual's pseudo-code reads differently | 4,000 DAA/DAS vectors from the physical chip, all pass; the RTL passes them too |
