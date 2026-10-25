# i8086 code generation

How `llc -mtriple=i8086` selects instructions. The ABI (registers, calling convention, frames) is in [abi.md](abi.md); the instruction set and encodings are in [isa.md](isa.md). This file describes the backend as it is (milestone 3); it names what is not done.

## Layers

1. **LLVM IR to a DAG, legalization.** i8 and i16 are the legal types. i1 is promoted, i32 and i64 are split into 16-bit pieces by the generic legalizer. There are no floating-point types.
2. **Selection** into the `P_*` instructions of `I8086InstrCodeGen.td` (TableGen patterns, plus `I8086ISelDAGToDAG.cpp` for the memory-operand matcher and the fixed-register instructions).
3. **Register allocation** over `GR8`, `GR16` and their subclasses.
4. **Frame lowering**: prologue, epilogue, callee-saved pushes, stack-argument cleanup (`I8086FrameLowering.cpp`).
5. **Lowering to machine instructions** (`I8086MCInstLower.cpp`): each `P_X` becomes the machine instruction `X` of `I8086InstrInfo.td`, which the assembler and object writer already encode and the disassembler decodes.

The machine instructions have no register semantics (every operand is an input, in assembly order), so selection cannot target them directly. A `P_X` has outputs, tied operands and implicit flags. The lowering collapses a tied operand into one, picks the shortest immediate encoding (sign-extended byte, accumulator, or general), and normalises memory operands. No `P_X` is ever decoded or assembled.

## Registers

| Class | Registers | Used for |
|---|---|---|
| `GR8` | AL CL DL BL AH CH DH BH | i8 values |
| `GR16` | AX CX DX BX SI DI (SP, BP, segment registers and FLAGS are reserved) | i16 values and pointers |
| `GR16_ABCD` | AX CX DX BX | 16-bit values whose halves are used (truncate, zero-extend) |
| `GR16_PTR` | BX SI DI BP | a lone address register |
| `GR16_BASE`, `GR16_INDEX` | BX BP; SI DI | the two registers of a `[base+index]` address |

Fixed-register instructions are modelled with implicit uses and defs: shifts by a variable count use CL, `cbw` reads AL and writes AX. Selection puts the operand in the register with a copy before the instruction; the allocator removes the copy when it can.

## Memory operands

A memory reference is four operands: base, index, displacement (a constant or a symbol plus offset), segment (always absent: CS = DS = ES = SS). `SelectAddr` decomposes an address into at most one frame index or symbol, one constant, and up to two registers. With two registers they are constrained to `GR16_BASE` and `GR16_INDEX`, which makes every combination a valid `[bx+si]`, `[bx+di]`, `[bp+si]` or `[bp+di]`. A lone register is `GR16_PTR`; the lowering moves SI or DI to the index slot, since the hardware has no `[si]` with SI as base. Anything that does not fit (two symbols, a symbol with a frame index, an out-of-range constant) is computed into a register.

## Control flow

There is no `setcc` and no conditional move. A comparison sets the flags (`CMP`, or `TEST r, r` for a compare with zero) and the glued branch reads them. A `select` and a boolean value become a diamond: `j<cc>` over a copy, joined by a PHI (`EmitInstrWithCustomInserter`). The hardware condition code is the low nibble of the Jcc opcode (`I8086CC`). Branches are emitted in the relaxable form, so the assembler picks a two-byte short branch or the five-byte inverted-jump-over-near-jump (see [asm_syntax.md](asm_syntax.md)).

## Arithmetic

- add, sub, and, or, xor, neg, not at 8 and 16 bits; `inc`/`dec` for a 16-bit increment of one; folded loads (`add ax, [mem]`).
- Shifts: a count of one is `shl r, 1`; a count of two is two such shifts; any other count goes through CL (the count is moved to CL first). A count of zero is dropped. A logical shift of a word by eight is two byte moves (`mov al, ah; mov ah, 0`).
- Multiply, divide and remainder at 16 and 32 bits, and 32-bit shifts, are calls to the runtime helpers (`__mulhi3`, `__divhi3`, ..., `__ashlsi3`). The runtime library is not written yet, so a program using them does not link (milestone 4). Native `mul`/`div` are a later optimisation.
- 32-bit and 64-bit add and subtract are expanded by the generic legalizer with compare-and-select carries; the add-with-carry instructions are not used yet.
- i8 to i16: zero-extend writes the high byte of the same register (`mov ah, 0`, also for `and x, 255`), sign-extend uses `cbw` (the byte is moved to AL), truncate reads the low half of a `GR16_ABCD` register.
- Absolute addresses with AL or AX use the short `mov al/ax, [addr]` forms, the same choice the assembler makes, so `llc -filetype=obj` and `llc | llvm-mc` produce identical objects (checked by `scripts/gen_run_objects.sh`).

## Verification

`tests/codegen/*.ll` check the shape of the output with FileCheck. `tests/run/*.ll` are compiled to objects, linked, and called on the 8088 core of `system/ibm_pc/emulator` with the ABI of [abi.md](abi.md); every result is compared with the same computation on the host over edge values and a fixed pseudo-random sequence. The tests also check that SP, BX, SI, DI and BP come back unchanged, so a callee-saved register cannot be clobbered unnoticed. A dedicated check found one real defect during bring-up (frame indices of incoming arguments are negative; a "no frame index" sentinel of -1 made stack arguments read from address 0).

## What is not done

Variadic functions and `byval` arguments are rejected with a diagnostic. There is no 32-bit add-with-carry, no native multiply/divide, no `rep movs`/`stos` for memory operations (small constant copies are loads and stores, larger ones call `memcpy`), no `loop`/`jcxz` selection, no `in`/`out` intrinsics, and no interrupt-handler attribute.
