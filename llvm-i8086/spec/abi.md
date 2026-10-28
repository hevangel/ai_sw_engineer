# i8086 ABI

Written before any backend code. the target `i8086-unknown-none` of the rustc in `rust/patches/` (data layout, `c_int` width) must match it exactly.

Status of each decision: **implemented** (with a date) once `llc` produces code that runs on the emulator (`tests/run/`); **proposed** until then. Decisions are numbered for reference; a title that links will freeze them.

## Data layout

- D1. LLVM data layout string: `e-m:e-p:16:16-p1:32:16:16:16-i8:8-i16:16-i32:16-i64:16-f32:16-f64:16-n8:16-a:8-S16`. The same shape as LLVM's 16-bit MSP430 target (which is why the Rust route works), plus address space 1: the far data pointer of D7 (32 bits, a 16-bit offset part). The 8088 has no alignment penalty on its 8-bit bus, so alignment is for the 16-bit 8086, and `a:8` keeps aggregates byte-aligned to save space.
- Pointer width / address spaces: 16-bit near pointers into a single 64 KiB segment (tiny/small model, CS = DS = ES = SS). No far address space in the first model (decision D2: segment registers are set once at entry and are not allocatable).
- Native integer widths: 8 and 16 (`n8:16`). 32-bit and 64-bit integers are legalized into 16-bit pieces; 128-bit integers add, shift and compare in pieces and multiply and divide through libcalls; `float` and `double` are software floating point (D6).
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

- Arguments: the first three 16-bit scalar arguments in AX, DX, CX; further arguments pushed on the stack right to left, 2-byte slots. Bytes and booleans travel as 16-bit values (extended as the attribute says, any-extended otherwise). A 32-bit argument is two such pieces, low word first, taken from the same sequence, so the first i32 is AX (low) and DX (high), and a second i32 has its low word in CX and its high word on the stack. Aggregates are passed by pointer by the frontend, or with `byval`: the caller copies the object onto the stack (rounded up to whole words) at the argument's place in the right-to-left order, and the callee uses that memory as its own copy (D3a).
- Return values: 16-bit in AX, 32-bit in DX:AX (DX high); anything that does not fit the two return registers is returned through a caller-supplied pointer (`sret`) passed as the first argument.
- Callee-saved: BX, SI, DI, BP. Caller-saved: AX, CX, DX, FLAGS. The direction flag is clear at calls and returns.
- Stack: full descending, SS:SP, a PUSH decrements SP by 2, SP kept even at call boundaries; the caller removes stack arguments (`add sp, n`). Depth is limited only by RAM; the linker script places the stack top at `_stack_top` below the data.
- Interrupt handlers: see D5 below.

### Variadic functions and `byval` (implemented 2026-10-27, D3a)

- **A call through a variadic function type passes every argument on the stack**, the named ones too: the callee cannot know which of AX, DX, CX the caller meant. Slots are 2 bytes, pushed right to left, as above (a byte is a word with the value in the low byte; an i32 is two words, low first; an i64 four). A function with `...` in its type receives all its arguments on the stack the same way.
- `va_list` is a pointer (16 bits). `va_start` stores the address of the first slot after the named arguments (`[bp + 4 + 2 * named_slots]`). `va_arg` reads the value at the pointer and advances it by the value's size rounded up to a whole slot; there is no extra alignment, because the slot is 2 bytes. `va_copy` copies the pointer; `va_end` does nothing.
- `byval` arguments (including between register arguments: the register arguments still take AX, DX, CX in order and the object takes stack slots in its position) up to 32 bytes are copied by `push` of word loads from the source, highest word first (an odd last byte is pushed as a zero-extended word); bigger ones get room with `sub sp` and a block copy (`rep movs`). In the callee the argument is the stack memory itself, so changes stay in the callee's copy.
- Floating-point variadic arguments are the integer bit patterns of D6 in 2 or 4 slots (there is no promotion of `float`; that is the frontend's business).

### Interrupt handlers (implemented 2026-10-27, D5)

A function with the `"interrupt"` attribute, or the `msp430_intrcc` calling convention (what rustc gives `extern "msp430-interrupt"`), is an interrupt handler. It takes no arguments and returns nothing. The prologue is `push bp; mov bp, sp`, then AX, CX, DX, BX, SI, DI (whatever the handler or its callees use: all six, always), then DS and ES, then `push cs; pop ds; push cs; pop es; cld` (the handler runs in the tiny model whatever segment the interrupted program had; the direction flag is clear for the ABI). The epilogue is `lea sp, [bp-16]`, `pop es; pop ds`, the six registers in reverse, `pop bp`, `iret`. FLAGS come back with IRET. The handler runs on the interrupted program's stack. The CPU clears IF on entry; the handler does not set it. The handler is not preserved across calls for any caller (nothing calls it); the vector is installed by the program (the IVT is at 0000:0000, four bytes a vector: offset, segment).

### Floating point and 128-bit integers (implemented 2026-10-27, D6)

There are no floating-point registers or instructions. `f32` and `f64` are legalized to `i32` and `i64`: they travel exactly as those integers do (an `f32` argument is two words, low first, from the AX, DX, CX sequence; an `f64` is four words; an `f32` is returned in DX:AX, an `f64` through the `sret` pointer), and every operation is a call to the compiler-rt routine of the usual name (`__addsf3`, `__subdf3`, `__mulsf3`, `__divdf3`, `__eqsf2`, `__ltdf2`, `__unordsf2`, `__extendsfdf2`, `__truncdfsf2`, `__fixsfsi`, `__fixunsdfdi`, `__floatsisf`, `__floatundidf`, and the 128-bit forms `__fixsfti`, `__floattidf`...). The comparison routines return a 16-bit `int` (what compiler_builtins defines for a 16-bit target), so the backend declares the comparison libcall result type as i16. 128-bit multiply, divide, remainder and variable shifts are `__multi3`, `__divti3`, `__udivti3`, `__modti3`, `__umodti3`, `__ashlti3`, `__lshrti3`, `__ashrti3`; a 128-bit value is eight words (arguments on the stack after the registers, results through `sret`). The routines are Rust's `compiler_builtins`, compiled by our `llc` (`scripts/build_compiler_builtins.sh`, `rt/README.md`); `sqrt`, `sin` and the other libm functions are not provided.

### Far data pointers (implemented 2026-10-27, D7)

`ptr addrspace(1)` is a far pointer: 32 bits, **offset in the low word and segment in the high word** (so an argument is offset in AX and segment in DX, as any 32-bit value). Address space 0 stays the near, 16-bit pointer of the tiny model; code, stack and the program's own data are near. The layout's index width for address space 1 is 16, so pointer arithmetic changes the offset only and wraps inside the segment (as `ES:[bx+disp]` does on the chip); it never carries into the segment.

Before selection the pass `I8086FarPointers` takes every use apart: a load or store of an integer, floating-point or pointer type becomes inline assembly, one word (or a trailing byte) at a time, `push es ; mov es, seg ; mov r, word ptr es:[bx] ; pop es` (ES is saved because the compiler keeps ES equal to DS for the string instructions); `getelementptr` changes the offset word; `addrspacecast` from a near pointer takes the segment from DS and to a near pointer drops it; `memcpy`, `memset` and `memmove` with a far operand are a byte loop (at most 64 KiB; `memmove` compares the 20-bit linear addresses and copies from the end when the destination is above the source); structs and arrays are accessed element by element, integers up to 128 bits in word accesses; vectors element by element; constant expressions of far type (`addrspacecast (ptr @g to ptr addrspace(1))`) are expanded into instructions (the segment is DS). What stays is the pointer as a 32-bit value: arguments, results, `phi`, `select`, `icmp`, `ptrtoint`, `inttoptr`.

### Far code (implemented 2026-10-27, D8)

A function with the calling convention `cc 1000` is a far function: arguments and results are placed as for a near call (D3), but the return address is a CS:IP pair, so the function returns with `retf` and its stack arguments (and the variadic area) start four bytes above the saved BP instead of two. A call to it is made with `cc 1000` at the call site:

- direct: `call f@seg:f` (the far call with an immediate segment and offset, opcode 9Ah, which pushes CS and IP). The segment half is a `R_I8086_SEG16` relocation, so `f` may be in any segment the linker placed it in.
- through a `ptr addrspace(1)` function pointer (`call cc 1000 addrspace(1) ty %f(...)`): offset and segment are stored in a four-byte frame slot and the call is `call dword ptr [bp-n]` (`call far [slot]`), which pushes CS:IP itself. This is the way to a function in another segment.

Registers preserved across the call are the same as for a near call. The address of a function as a far pointer is `addrspacecast (ptr @f to ptr addrspace(1))`: offset `f`, segment `f@seg` (below). Tests: `tests/codegen/farcode.ll` (instruction sequences), `far_code` on the 8088 core (direct, indirect, more than three arguments, 32-bit values, calls in a loop).

### Several segments (implemented 2026-10-27, D9)

`sym@seg` (assembler syntax; `R_I8086_SEG16 = 5` in the object) is the segment the linker placed `sym` in, as a paragraph number counted from the program's first segment (paragraph 0 is the PSP of a DOS program, whose code starts at offset 100h). It is used as an immediate (`mov ax, sym@seg`), as the segment half of a far call (`call sym@seg:sym`) and as a data word (`.word sym@seg`). In IR the compiler writes it for `addrspacecast` of a pointer into a global (a variable or a function) from address space 0 to 1: the offset is the symbol's offset and the segment is `sym@seg`; for any other near pointer the segment is DS as before. A symbol's value in a program with several segments is its **offset within its own segment**.

The linker script gives a region a segment of its own: `far (rwx) : ORIGIN = 0, LENGTH = 0x4000, SEGMENT = AUTO` (or `SEGMENT = <paragraph>`); `ORIGIN` is then an offset in that segment and `AUTO` starts the segment at the first paragraph past what is placed before it. NOLOAD sections in such a region are stored as zeros in the file. A `PC8`/`PC16` relocation from one segment to another is an error ("use a far call"). `--exe` writes the MZ relocation table for every `SEG16` word (DOS adds the segment it loaded the program at; the stored words are relative to the load module, PSP + 10h) and takes the initial SP from the symbol `__stack_top` that the program defines (the stack must be in the first segment, below nothing else: the other segments are above it); `--com` refuses a program that uses `SEG16`.

Tests: `tests/mc/segment.s`, `asm/tests/segments.rs`, `several_segments` in `i8086_codegen.rs` (segments.ll on the 8088 core), and a two-segment `.EXE` (`tests/smoke_ir/seg_exe.ll`) run by the original PC DOS 1.00 on the RTL and on the Rust emulator with the same screen (`dos_runs_the_two_segment_exe_and_shows_what_the_rtl_showed`).

Not done: a far *data* object larger than 64 KiB (the compiler takes one segment's offset arithmetic); and what Rust cannot express. Rust cannot name address space 1, so far access from Rust is `hal_far_read16` and friends in `system/ibm_pc/platform`.

Inline assembly (used by the pass and available to C-like IR) takes the constraints `r` and `q` (any register of the operand's width), `a b c d S D` (AX, BX, CX, DX, SI, DI or their byte halves) and the `{reg}` form; operands print as register names.

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
