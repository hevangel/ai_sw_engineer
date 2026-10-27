# i8086 code generation

How `llc -mtriple=i8086` selects instructions. The ABI (registers, calling convention, frames) is in [abi.md](abi.md); the instruction set and encodings are in [isa.md](isa.md). This file describes the backend as it is (milestone 3); it names what is not done.

## Layers

1. **LLVM IR to a DAG, legalization.** i8 and i16 are the legal types. i1 is promoted, i32 and i64 are split into 16-bit pieces by the generic legalizer. `float` and `double` are softened to `i32` and `i64` with libcalls (abi.md D6); `half`, `bfloat` and `fp128` are not supported.
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
- Multiply: a 16-bit product is `mul` (`imul` for the signed high half): the first operand is copied to AX, the other is a register, the product is DX:AX; the low half is the same for both. An 8-bit multiply is promoted to 16 bits. A 32-bit product is built from three 16-bit full products (no call). Constant multiplies are shifts and adds where the generic combiner finds them cheap, otherwise `mov cx, imm; mul cx`.
- Divide and remainder at 16 bits: `div` (DX cleared first) and `idiv` (after `cwd`) on DX:AX; one instruction gives the quotient (AX) and the remainder (DX), and the combiner merges a divide and a remainder of the same operands. 8-bit forms are promoted. A divide by a constant is a multiply by a reciprocal and shifts, or a shift for a power of two. As in LLVM IR, a divide by zero or `INT_MIN / -1` is undefined here; on the chip it raises interrupt 0.
- 32-bit and 64-bit divide and remainder, variable shifts of 32 and 64 bits, and 64-bit multiply are calls to the runtime helpers (`__divsi3`, `__udivsi3`, `__modsi3`, `__umodsi3`, `__ashlsi3`, `__lshrsi3`, `__ashrsi3`, and the `di` versions, `__muldi3`). They are in `rt/` (see [../rt/README.md](../rt/README.md)).
- Add and subtract with carry: the 16-bit pieces of a wider value are `add`/`adc` and `sub`/`sbb`, with the carry kept in the flags between them (a 32-bit add is `add ax, cx` then `adc dx, ...`, a 64-bit add is one `add` and three `adc`). A carry that is used as a value (the overflow intrinsics, a saturating add) is a boolean built with a branch; a boolean turned back into a carry is `add r8, 0xFF`, and a DAG combine removes that round trip when the boolean was just made from the previous piece's flags.
- i8 to i16: zero-extend writes the high byte of the same register (`mov ah, 0`, also for `and x, 255`), sign-extend uses `cbw` (the byte is moved to AL), truncate reads the low half of a `GR16_ABCD` register.
- Absolute addresses with AL or AX use the short `mov al/ax, [addr]` forms, the same choice the assembler makes, so `llc -filetype=obj` and `llc | llvm-mc` produce identical objects (checked by `scripts/gen_run_objects.sh`).

## Memory operations

A `memcpy` or `memset` that is too large to unroll as loads and stores (more than four of them) is a block instruction: `rep movsb`/`rep movsw` (CX = count, DI = destination, SI = source) and `rep stosb`/`rep stosw` (CX, DI, and AL or AX). Words are used when the size is a known even number and both ends are known to be 2-byte aligned; a word fill of a variable byte builds the byte in both halves of AX. The direction flag is clear (the ABI), so the copies go forward; `memmove` stays a call, because its overlap rule needs the direction decided at run time. No segment override is used, which also avoids the 8086/8088 erratum in which an interrupt during a `rep` with a segment override resumes without the override. The length operand may be an `i64` (LLVM makes one for the odd-sized tail of a struct initialiser): it is truncated to the 16-bit count (found compiling the CHKDSK title; `tests/codegen/memops.ll`).

## Verification

`tests/codegen/*.ll` check the shape of the output with FileCheck. `tests/run/*.ll` are compiled to objects, linked, and called on the 8088 core of `system/ibm_pc/emulator` with the ABI of [abi.md](abi.md); every result is compared with the same computation on the host over edge values and a fixed pseudo-random sequence. The tests also check that SP, BX, SI, DI and BP come back unchanged, so a callee-saved register cannot be clobbered unnoticed. A dedicated check found one real defect during bring-up (frame indices of incoming arguments are negative; a "no frame index" sentinel of -1 made stack arguments read from address 0).

## What is not done

There is no `memmove`/`memcmp` inline code (both are calls), no `loop`/`jcxz`/`xlat` selection, no `in`/`out` intrinsics (port access is assembly in the platform HAL). Multiply and divide of 32 and 64 bits, and variable shifts of those, are calls into `rt/`.
