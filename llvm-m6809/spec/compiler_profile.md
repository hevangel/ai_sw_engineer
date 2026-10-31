# MC6809 compiler contract (stage-1 specification)

For hevangel/ai_engineer#62. Sources: Motorola MC6809 data sheet and the
M6809 Programming Manual, cross-checked against MAME's m6809 core and the
project's MC6809 RTL (whose primary verification, the reyco2000 suite, was
hardware-validated).

## Exact variant

**Plain MC6809.** The Hitachi HD6309 superset (extra registers, DIV, 32-bit
ops) and the 68HC11 derivative are out of scope; per-machine profiles pin
6809 vs 68A09 clock grades only.

## Machine model (what makes the 6809 a compiler target)

- Registers: A/B (concatenated as **D** for 16-bit ops), index registers
  **X** and **Y**, user stack pointer **U**, system stack pointer **S**,
  direct-page register **DP**, PC, CC (E, F, H, I, N, Z, V, C).
- The double index register + `LEAX/LEAY/LEAS/LEAU` (load-effective-address
  with the full indexed mode) make the 6809 the first 8-bit CPU with real
  address arithmetic — frame-pointer codegen (`lea s,offset` prologues) is
  natural here, unlike the 6800.
- **Addressing modes to qualify (the issue names these)**: inherent,
  immediate, direct (DP-relative, one byte), extended (16-bit), and the
  five-byte-postbyte indexed set — no-offset, 5-bit signed offset,
  8/16-bit signed offset, A/B/D register offset, each on X/Y/U/S, each
  with indirect variants plus auto-increment/decrement by 1/2 — and
  8/16-bit PC-relative branches (short and long, position-independent).
  The postbyte encoder table is the 6809-specific MC-layer work item.
- 16-bit data: D register ops (ADDD/SUBD/CMPD/LDD/STD), `MUL` (8×8→16
  unsigned), 16-bit loads/stores (big-endian in memory), `SEX/EXT`… sign
  extension via SEX; no hardware divide.
- Interrupts (runtime-visible): FIRQ with a minimal frame (CC+PC only),
  IRQ/SWI/SWI2/SWI3 with full frames, E-bit-driven restore; CWAI/SYNC as
  wait/stop primitives.

## Data layout

Flat 64 KiB, **big-endian** (MSB at the lower address), like the 6800.
Proposed data layout string family: `E-m:e-p:16:8-a:8` (exact string at
M0). Direct page: DP is a live register — compiler policy candidates are
(a) set-and-forget DP=0 with direct mode as the zero page, or (b) DP as an
allocatable base for position-independent globals; gcc6809 uses (a)-style
defaults; freeze at M0 after measuring both on the smoke suite.

## ABI (planned; freeze at M0 in `spec/abi.md`)

- Reference point: gcc6809's documented convention (arguments on the
  system stack, S as call stack, U available as frame/emulated register
  file, D return value; X/Y clobbered), adjusted for Rust: `i8` in A or
  B, `i16` in D, pointers 16-bit in X/Y/D, `i32`/`i64` expanded in
  memory, soft-float only.
- Callee-saves via `PSHS`/`PULS` register-set pushes (a single instruction
  saving an arbitrary subset — a lowering nicety unique to the 6809);
  `PSHU`/`PULU` for U-frame saves.
- Runtime: crt0 (`SETS`/direct-page init, zero .bss, copy .data from ROM,
  `jsr main`, park), `libm6809rt` for divide, wider shifts and memcpy.

## Memory map and executable formats (reference machines)

- Reference profiles: a DECB-style Color Computer map (ROM at $8000–$FFFF,
  RAM from $0000 up) and an OS-9-style module map. Package formats the
  linker must emit: **Motorola S-records**, DECB binary blocks, and (as a
  follow-up) OS-9 module format with its CRC — the formats gcc6809/LWTOOLS
  already produce, so each has a cross-check reference.
- Vectors: RESET $FFFE/F, NMI $FFFC/D, SWI $FFFA/B, IRQ $FFF8/9, FIRQ
  $FFF6/7, SWI2 $FFF4/5, SWI3 $FFF2/3 (verified against MAME and the RTL's
  suite ROMs).

## Out of scope for this profile

- HD6309 and 68HC11 supersets; 6800 (issue #61); 68k (issue #64).
- Application/OS rewrites, including an OS-9 port (its own future issue).
