# llvm-i8086 report

Status (2026-10-25): milestones 2 (the machine-code layer), 3 (code generation for integer code at 8 and 16 bits) and 4 (native multiply/divide, add-with-carry, block instructions) are done. `llc -mtriple=i8086` compiles LLVM IR to assembly and ELF objects that run on the 8088 core. There is no runtime library, no linker, no Rust target that builds, and no title yet.

| Check | Result |
|---|---|
| Build | `scripts/build_llvm.sh` builds `llc`, `llvm-mc`, `llvm-objdump`, `llvm-readobj`, `llvm-tblgen`, `FileCheck`, `not`, `count` with the experimental `I8086` target (about 8 minutes cold with ccache; a target-only change rebuilds in about a minute) |
| MC lit suite | 7 of 7 files pass: `encodings.s` (423 rows from the manual's opcode structure, 36 branch encodings with fixups), `disasm.txt`, `disasm-invalid.txt`, `relax.s`, `range.s`, `fixups.s`, `errors.s` |
| Codegen lit suite | 7 of 7 files pass (`tests/codegen/`): ALU, branches and selects, addressing modes and frames, calls, 32/64-bit values and carries, multiply and divide, block operations |
| Execution on the 8088 core | 10 of 10 programs pass (`tests/run/`, run by `system/ibm_pc/emulator/tests/i8086_codegen.rs`): 150 functions (arithmetic, comparisons, loops, shifts, extensions, globals and pointers, stack objects, calls with stack arguments, recursion, 32- and 64-bit arithmetic with carries, multiply and divide, block copies and fills), each called with hundreds to thousands of input combinations and compared with host arithmetic; SP, BX, SI, DI and BP are checked on every return. Two deliberately wrong expectations fail as they should |
| Text and object routes agree | `llc | llvm-mc` and `llc -filetype=obj` give identical sections for every `tests/run` program (`scripts/gen_run_objects.sh --check`) |
| Physical-chip corpus (`tests/mc/vector_check.py`) | 8,370 instruction instances: all decode with MartyPC's mnemonic and no leftover bytes; 7,294 re-assemble byte for byte, 206 to an alternative encoding that disassembles identically, 870 relative branches checked for decoding only; 0 problems |
| Smoke programs on a whole machine, the Rust target, titles | not applicable yet |

## What exists

- `llvm-mc -triple=i8086`: Intel-syntax assembler and disassembler for the whole 8086 integer instruction set (no 80186+ and no undocumented opcodes), memory operands in all eight base/index forms with 8- and 16-bit displacements, segment overrides, prefixes, string instructions, far `jmp`/`call`, port I/O. Branch relaxation. ELF32 objects with `e_machine` 0x8086 and the relocation kinds of `spec/abi.md`.
- `llc -mtriple=i8086`: selection for i8 and i16 ALU, loads and stores with every addressing mode (`[bx+si+4]`, `[bp+si-10]`, `[sym+bx]`, `[sym]`), compares and conditional branches, selects, shifts, extensions, native 16-bit `mul`/`div`, calls with the ABI of `spec/abi.md` D3, frames (D4), callee-saved registers; i32 and i64 through the generic legalizer, with `add`/`adc` and `sub`/`sbb` chains and a 32-bit product from three `mul`s; `rep movs`/`stos` for large copies and fills. See `spec/codegen.md`.
- The target description (`I8086*.td`): the machine instructions with their encoding in TSFlags (shared by the emitter, the decoder and the assembler) and the code generation instructions (`I8086InstrCodeGen.td`) that selection targets and `I8086MCInstLower.cpp` turns into them.

## How it was verified

The encoding table was written by hand from the structure of Intel's opcode map and kept apart from the implementation. The corpus check uses an independent source neither the table nor the backend derives from: bytes and disassembly of random instances captured from a real 8086 by the SingleStepTests project, cached by the hardware project. Writing the corpus check found the one bug the unit tables missed (the decoder did not count the fixed second byte of `aam`/`aad`).

Generated code is checked by running it. The emulator's 8088 core is itself verified against the physical-chip vectors and the RTL; the expected values are the host's integer arithmetic, so neither the compiler nor the core is its own oracle. Bring-up found these defects, all fixed: nested `EXTRACT_SUBREG` results without a type are silently dropped by TableGen (a `trunc` pattern never matched); a frame-index sentinel of -1 collided with the negative indices of incoming-argument objects (stack arguments read from address 0); the object writer and the text route chose different encodings for `mov ax, [abs]`; the carry of a wide add went through a branch because the combine that removes the flag-to-boolean round trip did not exist yet (a custom lowering sees the carry before its producer is lowered). The test's own mistakes (an out-of-contract `zeroext` argument, a halted CPU kept across calls) were found the same way.

## Known gaps

- 32- and 64-bit divide and remainder, variable shifts of those, 64-bit multiply and large `memmove`/`memcmp` are calls to helpers that are not written (`__divsi3`, `__ashlsi3`, `__muldi3`, ...; names in `spec/abi.md`). A program using them does not link.
- No `loop`/`jcxz`/`xlat` selection, no `in`/`out` intrinsics, no variadic functions, no `byval`, no interrupt attribute, no floating point.
- Code quality is first-order: a compare with a boolean result builds the value with a branch; a shift by eight of a value in a register pair does not reuse the same register; no peephole pass.
- No linker, no image packager and no Rust target (`rust/i8086-none.json`) yet: milestones 6 and 7. The test linker in the emulator's test file is a stand-in.
- Intel manual page numbers for the table rows are still to be added; the corpus is the authority until then.
- `esc` (coprocessor escapes) and the `0F`/80186+ and undocumented opcodes are rejected on purpose.
- A label named `short`, `near` or `far` is read as a keyword.
