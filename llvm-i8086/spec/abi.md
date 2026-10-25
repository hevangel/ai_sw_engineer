# i8086 ABI

Written before any backend code. `rust/i8086-none.json` must match it exactly.

Status of each decision: **proposed** until the first function lowers and a title links, then frozen here with a date (AGENTS: design docs describe the backend as it is). Decisions are numbered for reference.

## Data layout

- D1. LLVM data layout string: `e-m:e-p:16:16-i8:8-i16:16-i32:16-i64:16-f32:16-f64:16-n8:16-a:8-S16`. Same shape as LLVM's 16-bit MSP430 target. The 8088 has no alignment penalty on its 8-bit bus, so alignment is for the 16-bit 8086, and `a:8` keeps aggregates byte-aligned to save space.
- Pointer width / address spaces: 16-bit near pointers into a single 64 KiB segment (tiny/small model, CS = DS = ES = SS). No far address space in the first model (decision D2: segment registers are set once at entry and are not allocatable).
- Native integer widths: 8 and 16 (`n8:16`). 32-bit and 64-bit integers are legalized into 16-bit pieces; no floating point.
- Endianness: little.

## Registers

| Class | Registers | Allocatable | Notes |
|---|---|---|---|
| GR16 | AX, BX, CX, DX, SI, DI | yes | BX, SI, DI, BP are the only base/index registers in addressing modes; AX, CX, DX are fixed by MUL/DIV, shifts, string and I/O instructions |
| GR8 | AL AH BL BH CL CH DL DH | yes (aliases of AX..DX) | |
| BP | BP | frame pointer when needed | otherwise allocatable as a base register |
| SP | SP | no | stack pointer |
| SEG | CS DS ES SS | no | fixed by the tiny model |
| FLAGS | FLAGS | no (implicit) | |

Imaginary registers (memory-backed pseudo-registers): none planned. The 8086 has enough registers that the pressure of `llvm-i8008` and `llvm-i4004` does not apply.

## Calling convention (proposed, D3)

- Arguments: the first three 16-bit scalar arguments in AX, DX, CX; further arguments pushed on the stack right to left, 2-byte slots. A 32-bit argument uses a register pair taken from that sequence. Aggregates larger than 4 bytes are passed by pointer.
- Return values: 16-bit in AX, 32-bit in DX:AX (DX high), larger values by a caller-supplied pointer (sret) passed as the first argument.
- Callee-saved: BX, SI, DI, BP. Caller-saved: AX, CX, DX, FLAGS.
- Stack: full descending, SS:SP, a PUSH decrements SP by 2, SP kept even at call boundaries; the caller removes stack arguments (`ADD SP,n`). Frame pointer is BP only for functions with dynamic stack allocation or when requested. Depth is limited only by RAM; the linker script places the stack top at `_stack_top` below the data.
- Interrupt handlers use a separate attribute (IRET, save everything) and are out of scope for the first model.

## Relocations and object format (implemented, 2026-10-25)

ELF32 little-endian relocatable objects (RELA) with `e_machine` = **0x8086** (`EM_I8086`; a private value, no standard number is assigned), `llvm-readobj` names it "Intel 8086/8088". Relocation types (`llvm/BinaryFormat/ELFRelocs/I8086.def`, installed into the LLVM tree by `scripts/apply_overlay.py`):

| Type | Value | Field | Computation |
|---|---|---|---|
| `R_I8086_NONE` | 0 | | |
| `R_I8086_8` | 1 | 1 byte | S + A (an immediate or `.byte` naming a symbol) |
| `R_I8086_16` | 2 | 2 bytes, little endian | S + A (an offset within the segment: `mov ax, sym`, `[sym]`, `.word sym`) |
| `R_I8086_PC8` | 3 | 1 byte, signed | S + A - P, A includes -1 for the field bias (short `jmp`, `jcc`, `loop`, `jcxz`) |
| `R_I8086_PC16` | 4 | 2 bytes, signed | S + A - P, A includes -2 (near `jmp`, `call`, and the `jmp` of a relaxed `jcc`) |

The assembler relaxes a bare short branch to the near form when its target is an external symbol (the linker cannot make a long displacement fit a byte). The linker writes a flat binary at the platform load address (see `system/ibm_pc/platform/link.ld`); `.EXE`/`.COM` headers are a later packager. Far (segment:offset) references are not relocated: segments are constants in the first model.

## Runtime library

Native instructions cover 8- and 16-bit multiply and divide (`MUL`, `IMUL`, `DIV`, `IDIV`), shifts by 1 or by `CL`, and `REP MOVS`/`REP STOS`. Helpers are needed for 32- and 64-bit multiply, divide and shifts, for 16-bit shifts by a variable count where a loop is smaller than `CL` use, and for `memcmp`/`bcmp`. They live in `rt/` as assembly or no_std Rust, are linked on demand, and each has a contract recorded here when it exists.
