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

- Arguments: the first three 16-bit scalar arguments in AX, DX, CX; further arguments pushed on the stack right to left, 2-byte slots. Bytes and booleans travel as 16-bit values (extended as the attribute says, any-extended otherwise). A 32-bit argument is two such pieces, low word first, taken from the same sequence, so the first i32 is AX (low) and DX (high), and a second i32 has its low word in CX and its high word on the stack. Aggregates are passed by pointer by the frontend, or with `byval`: the caller copies the object onto the stack (rounded up to whole words, at most 128 bytes) at the argument's place in the right-to-left order, and the callee uses that memory as its own copy (D3a).
- Return values: 16-bit in AX, 32-bit in DX:AX (DX high); anything that does not fit the two return registers is returned through a caller-supplied pointer (`sret`) passed as the first argument.
- Callee-saved: BX, SI, DI, BP. Caller-saved: AX, CX, DX, FLAGS. The direction flag is clear at calls and returns.
- Stack: full descending, SS:SP, a PUSH decrements SP by 2, SP kept even at call boundaries; the caller removes stack arguments (`add sp, n`). Depth is limited only by RAM; the linker script places the stack top at `_stack_top` below the data.
- Interrupt handlers use a separate attribute (IRET, save everything) and are out of scope for the first model.

### Variadic functions and `byval` (implemented 2026-10-27, D3a)

- **A call through a variadic function type passes every argument on the stack**, the named ones too: the callee cannot know which of AX, DX, CX the caller meant. Slots are 2 bytes, pushed right to left, as above (a byte is a word with the value in the low byte; an i32 is two words, low first; an i64 four). A function with `...` in its type receives all its arguments on the stack the same way.
- `va_list` is a pointer (16 bits). `va_start` stores the address of the first slot after the named arguments (`[bp + 4 + 2 * named_slots]`). `va_arg` reads the value at the pointer and advances it by the value's size rounded up to a whole slot; there is no extra alignment, because the slot is 2 bytes. `va_copy` copies the pointer; `va_end` does nothing.
- `byval` arguments (including between register arguments: the register arguments still take AX, DX, CX in order and the object takes stack slots in its position) are copied by `push` of word loads from the source, highest word first; an odd last byte is pushed as a zero-extended word. In the callee the argument is the stack memory itself, so changes stay in the callee's copy.
- Not done: a variadic call passes floating point as nothing (there is no floating point), and a `byval` object above 128 bytes is rejected by the backend with a diagnostic (a loop would be needed).

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

The assembler relaxes a bare short branch to the near form when its target is an external symbol (the linker cannot make a long displacement fit a byte); a branch to another section of the same file is relaxed the same way, because section placement is not known when the file is assembled.

## Linking and images (implemented 2026-10-25)

`i8086-ld` ([../asm/README.md](../asm/README.md)) applies the relocations above with these checks: `R_I8086_8` fits -128..255, `R_I8086_16` fits -32768..65535, `R_I8086_PC8` fits -128..127 (an error otherwise, naming the place), `R_I8086_PC16` is taken modulo 2^16 because a near jump or call reaches anywhere in the segment. Far (segment:offset) references are not relocated: segments are constants in the first model.

Section names the platform scripts rely on: `.boot` (the boot sector, `KEEP`), `.text`, `.text.*`, `.rodata*`, `.data*`, `.bss*` (NOLOAD). The compiler's `-ffunction-sections`/`-fdata-sections` behaviour (the backend sets it) gives every function and global its own section, so the linker's garbage collection removes what a program does not use. `system/ibm_pc/platform/link.ld` is the layout: the boot sector at 0000:7C00 (512 bytes, ends with `55 AA`), the rest of the image from 7E00 (loaded by the boot sector's own INT 13h read), `.bss` and data below the boot sector from 0500, the stack growing down from 7C00 (`_stack_top`). The flat binary is the loaded sections from 7C00 up; the floppy image is that binary padded to 163,840 bytes. `.EXE`/`.COM` headers are a later packager.

## Runtime library

Native instructions (implemented 2026-10-25) cover 16-bit multiply and divide (`MUL`, `IMUL`, `DIV`, `IDIV` on AX and DX), the 32-bit product (from three 16-bit full products), add and subtract with carry for 32- and 64-bit values, shifts by 1 or by `CL`, and `REP MOVS`/`REP STOS` for block copies and fills above four loads and stores.

The helpers (implemented 2026-10-25, [../rt/README.md](../rt/README.md) has the contracts) are called by the libgcc names the backend registers (`I8086Subtarget::initLibcallLoweringInfo`): `__divsi3`, `__udivsi3`, `__modsi3`, `__umodsi3` (32-bit divide and remainder), `__ashlsi3`, `__lshrsi3`, `__ashrsi3` (32-bit shifts by a variable), `__mulsi3`, `__muldi3`, `__divdi3`, `__udivdi3`, `__moddi3`, `__umoddi3`, `__ashldi3`, `__lshrdi3`, `__ashrdi3` (64-bit), and `memcpy`, `memset`, `memmove`, `memcmp`/`bcmp`. They live in `rt/` as assembly (32-bit, memory) or LLVM IR (64-bit), one section per routine, are linked on demand, and follow the calling convention above. Not provided: the 16-bit helpers (`__mulhi3`, `__divhi3`, ...), because 16-bit multiply and divide are single instructions, and floating point.
