# MC68000 compiler profile (stage-1 specification)

For hevangel/ai_engineer#64. Sources: Motorola MC68000 data sheet/Programmer's
Reference Manual, the LLVM M68k target's Rust target spec, and gcc m68k's
PSABI as the calling-convention reference.

## Exact variant

Plain **MC68000** ISA: the build pins `-C target-cpu=M68000` (the tier-3
target defaults to M68010, which permits 68010-only forms such as `RTD`/
`MOVEC`-adjacent opcodes). Later-CPU-only instructions are banned and
checked (`disasm_check.py` bans `movec`, `rtd`, `moves`, `cas`, `bf*`,
`pack`, `unpk`, `chk2`/`cmp2`, `callm`, and friends). 68020/030/CPU32
profiles are separate decisions for machine issues.

## Data layout

- Big-endian; `data-layout E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16` (the target's own).
- 32-bit `int`/pointers (the m68k PSABI convention), 24-bit external
  address space on the original part (programs are linked within the first
  16 MiB; the executor maps 16 MiB).

## ABI (as exercised; System V m68k conventions)

- Scratch: `d0`, `d1`, `a0`, `a1`; callee-saved: `d2–d7`, `a2–a6`;
  return value in `d0`; remaining arguments on the stack; frame via
  `link`/`unlk a6` when needed.
- Bare-metal entry: `_start` (no crt0 required for the smoke — the
  executor provides SSP/PC via the reset vector; rustc emits `move.l
  #…,sp`-free code and the runtime is `core` only).

## Memory map (qualification harness)

| Region | Address | Use |
|---|---|---|
| Reset vectors | `$00000000` | SSP (`$00020000`), PC (`$00000400`) |
| Program | `$00000400…` | linked `.text`/`.rodata`/`.data` (linked at `-Ttext=0x400`) |
| INPUT window | `$00001000` | 8 stimulus bytes written by the executor |
| OUTPUT window | `$00001100` | 32 result bytes the smoke writes |
| DONE sentinel | `$00001200` | `0xA5` on completion |
| Initial stack | `$00020000` | SSP from the reset vector, grows down |

Package formats: ELF (for `objdump`/`nm`) and flat binary
(`objcopy -O binary`) for the executor; a ROM-budget gate belongs to the
per-machine profiles (Mac/Amiga/ST own their maps).

## Runtime

`core` + `compiler_builtins` only (soft-float; m68k FPU profiles are
separate). `count_ones`-class ops lower to `compiler_builtins` helpers
(`jsr` + hardware stack) — exercised by the smoke.

## Out of scope for this profile

68010+ instructions (banned), FPU/MMU profiles, application/OS rewrites,
and machine-specific memory maps (machine issues own those).
