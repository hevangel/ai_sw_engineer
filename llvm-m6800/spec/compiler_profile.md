# MC6800 compiler contract (stage-1 specification)

For hevangel/ai_engineer#61. Sources: *M6800 Programming Reference Manual*,
M68PRM(D), November 1976 (bitsavers) — the ai_hw_engineer design's primary
source and this profile's oracle reference — cross-checked against the
project's MC6800 RTL.

## Exact variant

**MC6800, the plain 1974 part.** Not 6801/6301/68HC11 (which add D
accumulator, MUL, more I/O), not 6802/6808 (package/onboard-RAM variants of
the same core — acceptable only as mask-identical cores, decided per machine
profile). The issue forbids treating 6809 or 68k as interchangeable.

## Machine model

- Registers: accumulators **A** and **B** (8-bit), index register **IX**
  (16-bit), stack pointer **SP** (16-bit, hardware, grows down), **PC**
  (16-bit), condition code **CC** (H, I, N, Z, V, C).
- No hardware multiply/divide — all wider arithmetic is software
  (compiler-rt-style helper routines, as `libi4004rt`/`libi8008rt` do).
- Memory: flat 64 KiB, **big-endian** 16-bit quantities (MSB at the lower
  address — the data layout string will lead with `E` and 16-bit pointers
  at 8-bit alignment: `E-m:e-p:16:8-a:8` family, exact string at M0).
- Direct page $0000–$00FF: single-byte addressing — the natural zero-page
  region for globals/spills (the cost model must prefer it, like llvm-mos
  treats the zero page).
- Hardware stack frame: the 6800 pushes return addresses on the hardware
  stack (unlike the 4004's 3-level stack or the 8008's), so calls are
  `BSR/JSR` + `RTS`; reentrancy works but the frame pointer must be
  software-maintained (typically IX) because there is no base-pointer
  addressing — indexed addressing is IX-only with a signed 8-bit offset
  (−128..+127) or the 16-bit offset mode (3-byte instruction).

## ISA subset (compiler-visible)

72 instructions across six addressing modes (immediate, direct, extended,
indexed, inherent, relative). The CodeGen subset: LDAA/LDAB, STAA/STAB,
ADDA/ADDB (+carry via ADC), SUBA/SUBB/CMP, AND/ORAA/EOR/COM/NEG, INCA/DECA
and the IX equivalents, ASL/ASR/LSR/ROL/ROR, TBA/TAB/TAP/TPA, PSH/PUL,
BRA/BR*/BSR/JSR/JMP/RTS, CLC/CLI/SEC/SEI, WAI/SWI (as traps). 16-bit
operations (loads, adds, compares) expand to byte pairs with explicit
carry handling — the register-pair expansion problem llvm-i8008 solved;
the pair model will be fixed in `spec/abi.md` before TableGen.

## ABI (planned; freeze at M0 in `spec/abi.md`)

- Scalars: `i8` native in A/B; `i16`/pointers in register pairs where they
  fit, else memory. `i32`/`i64` expanded, soft-float only (no FPU exists).
- Calling convention candidate (to be validated against hand-written MIKBUG
  routines): first `i8` argument in A (second in B), 16-bit arguments in
  IX/memory, rest on the stack; caller-cleanup or callee-cleanup decided at
  M0 with the RTS/parameter model; A/B/CC caller-saved, IX callee-saved.
- Runtime: crt0 (reset: S to top of RAM, zero .bss, copy .data from ROM,
  `jsr main`), `libm6800rt` for pair arithmetic/shifts/memcpy.

## Memory map and executable format (reference board)

- Reference machine profile: the MIKBUG/MEK6800D2 class board — RAM at
  $0000, MIKBUG ROM at $E000–$FFFF, vectors $FFF8–$FFFF (MIKBUG's own
  entries occupy the vector area; a bare-metal image owns them directly).
  Exact per-machine maps belong to the machine issues.
- Package format: **Motorola S-records** (the board monitors' loader
  format), plus a flat binary option; the packer/linker emits both with a
  ROM-budget gate, in the `i4004_pack.py` style.

## Out of scope for this profile

- 6801/6301/68HC11 supersets, 6809 (issue #62's subject), 68k (issue #64).
- Application/OS rewrites (software issues own those).
