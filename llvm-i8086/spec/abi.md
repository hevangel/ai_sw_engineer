# i8086 ABI

Written before any backend code. `rust/i8086-none.json` must match it exactly.

Status of each decision: **implemented** (with a date) once `llc` produces code that runs on the emulator (`tests/run/`); **proposed** until then. Decisions are numbered for reference; a title that links will freeze them.

## Data layout

- D1. LLVM data layout string: `e-m:e-p:16:16-i8:8-i16:16-i32:16-i64:16-f32:16-f64:16-n8:16-a:8-S16`. Same shape as LLVM's 16-bit MSP430 target. The 8088 has no alignment penalty on its 8-bit bus, so alignment is for the 16-bit 8086, and `a:8` keeps aggregates byte-aligned to save space.
- Pointer width / address spaces: 16-bit near pointers into a single 64 KiB segment (tiny/small model, CS = DS = ES = SS). No far address space in the first model (decision D2: segment registers are set once at entry and are not allocatable).
- Native integer widths: 8 and 16 (`n8:16`). 32-bit and 64-bit integers are legalized into 16-bit pieces; no floating point.
- Endianness: little.

## Registers

| Class | Registers | Allocatable | Notes |
|---|---|---|---|
| GR16 | AX, BX, CX, DX, SI, DI | yes | BX (base) and SI, DI (index) address memory, BP too as a base for frames; AX, CX, DX are fixed by MUL/DIV, shifts, string and I/O instructions. Only AX..DX have byte halves |
| GR8 | AL AH BL BH CL CH DL DH | yes (aliases of AX..DX) | |
| BP | BP | no | the frame pointer (D4); also the base of every local and spill address |
| SP | SP | no | stack pointer |
| SEG | CS DS ES SS | no | fixed by the tiny model |
| FLAGS | FLAGS | no (implicit) | |

Imaginary registers (memory-backed pseudo-registers): none planned. The 8086 has enough registers that the pressure of `llvm-i8008` and `llvm-i4004` does not apply.

## Calling convention (implemented 2026-10-25, D3)

- Arguments: the first three 16-bit scalar arguments in AX, DX, CX; further arguments pushed on the stack right to left, 2-byte slots. Bytes and booleans travel as 16-bit values (extended as the attribute says, any-extended otherwise). A 32-bit argument is two such pieces, low word first, taken from the same sequence, so the first i32 is AX (low) and DX (high), and a second i32 has its low word in CX and its high word on the stack. Aggregates are passed by pointer by the frontend; `byval` is rejected by the backend.
- Return values: 16-bit in AX, 32-bit in DX:AX (DX high); anything that does not fit the two return registers is returned through a caller-supplied pointer (`sret`) passed as the first argument.
- Callee-saved: BX, SI, DI, BP. Caller-saved: AX, CX, DX, FLAGS. The direction flag is clear at calls and returns.
- Stack: full descending, SS:SP, a PUSH decrements SP by 2, SP kept even at call boundaries; the caller removes stack arguments (`add sp, n`). Depth is limited only by RAM; the linker script places the stack top at `_stack_top` below the data.
- Interrupt handlers use a separate attribute (IRET, save everything) and are out of scope for the first model.
- Variadic functions are not supported yet.

## Frames (implemented 2026-10-25, D4)

SP cannot address memory (there is no `[sp+disp]`), so a function with stack objects (locals, spills, incoming stack arguments) addresses its frame through BP, and BP is therefore reserved in every function. A function without stack objects has no frame pointer and no prologue.

```
push bp ; mov bp, sp        ; [bp] = saved BP, [bp+2] = return address, [bp+4] = first stack argument
push bx / si / di           ; those the function uses, in that order: [bp-2], [bp-4], [bp-6]
sub sp, N                   ; locals and spills, below the pushed registers
...
lea sp, [bp-2k]             ; k pushed registers (mov sp, bp when k = 0)
pop di / si / bx ; pop bp ; ret
```

Callee-saved registers are pushed, never stored below SP: an interrupt pushes onto the stack at SP and would overwrite anything under it. Locals are only addressed after `sub sp, N` has made room.

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
