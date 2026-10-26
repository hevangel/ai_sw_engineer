# llvm-i8086 report

Status (2026-10-25): milestones 2 (the machine-code layer), 3 (code generation for integer code at 8 and 16 bits), 4 (native multiply/divide, add-with-carry, block instructions), 5 (the runtime library) and 6 (the linker and image packager) are done. `llc -mtriple=i8086` compiles LLVM IR to assembly and ELF objects, `i8086-ld` links them with the runtime library and packs a bootable floppy, and the code runs on the 8088 core, including a boot through the original BIOS. There is no Rust target that builds and no title yet.

| Check | Result |
|---|---|
| Build | `scripts/build_llvm.sh` builds `llc`, `llvm-mc`, `llvm-objdump`, `llvm-readobj`, `llvm-tblgen`, `FileCheck`, `not`, `count` with the experimental `I8086` target (about 8 minutes cold with ccache; a target-only change rebuilds in about a minute) |
| MC lit suite | 7 of 7 files pass: `encodings.s` (423 rows from the manual's opcode structure, 36 branch encodings with fixups), `disasm.txt`, `disasm-invalid.txt`, `relax.s`, `range.s`, `fixups.s`, `errors.s` |
| Codegen lit suite | 7 of 7 files pass (`tests/codegen/`): ALU, branches and selects, addressing modes and frames, calls, 32/64-bit values and carries, multiply and divide, block operations |
| Execution on the 8088 core | 13 of 13 tests pass (`tests/run/` and `rt/obj/` linked by `i8086-ld`, run by `system/ibm_pc/emulator/tests/i8086_codegen.rs`): 150 compiled functions (arithmetic, comparisons, loops, shifts, extensions, globals and pointers, stack objects, calls with stack arguments, recursion, 32- and 64-bit arithmetic with carries, multiply and divide, block copies and fills) plus the runtime routines, each called with hundreds to thousands of input combinations and compared with host arithmetic; SP, BX, SI, DI and BP are checked on every return. Two deliberately wrong expectations fail as they should |
| Text and object routes agree | `llc | llvm-mc` and `llc -filetype=obj` give identical sections for every `tests/run` program (`scripts/gen_run_objects.sh --check`) |
| Physical-chip corpus (`tests/mc/vector_check.py`) | 8,370 instruction instances: all decode with MartyPC's mnemonic and no leftover bytes; 7,294 re-assemble byte for byte, 206 to an alternative encoding that disassembles identically, 870 relative branches checked for decoding only; 0 problems |
| Linker | `asm/`: 19 tests on objects built in the test (placement, alignment, each relocation and its range, garbage collection, weak/duplicate/undefined symbols, region overflow, bss, `/DISCARD/`, the ELF output, the floppy checks, script errors with line numbers, the IBM PC script); `tests/boot/hello.o` linked with the platform script and its `llvm-objdump` disassembly checked with FileCheck |
| Boot image | `system/ibm_pc/emulator/tests/i8086_boot.rs`: the layout of the packed disk, stage 2 run from the linked binary on the 8088 core (counter in .bss, text on the CGA screen), and the whole boot through the original BIOS (the BIOS reads the boot sector, INT 13h reads sector 2, stage 2 runs); wrong expectations fail as they should |
| Smoke programs from Rust, the Rust target, titles | not applicable yet |

## What exists

- `llvm-mc -triple=i8086`: Intel-syntax assembler and disassembler for the whole 8086 integer instruction set (no 80186+ and no undocumented opcodes), memory operands in all eight base/index forms with 8- and 16-bit displacements, segment overrides, prefixes, string instructions, far `jmp`/`call`, port I/O. Branch relaxation. ELF32 objects with `e_machine` 0x8086 and the relocation kinds of `spec/abi.md`.
- `llc -mtriple=i8086`: selection for i8 and i16 ALU, loads and stores with every addressing mode (`[bx+si+4]`, `[bp+si-10]`, `[sym+bx]`, `[sym]`), compares and conditional branches, selects, shifts, extensions, native 16-bit `mul`/`div`, calls with the ABI of `spec/abi.md` D3, frames (D4), callee-saved registers; i32 and i64 through the generic legalizer, with `add`/`adc` and `sub`/`sbb` chains and a 32-bit product from three `mul`s; `rep movs`/`stos` for large copies and fills. See `spec/codegen.md`.
- `i8086-ld` (`asm/`): section placement into the regions of a linker script, section garbage collection, the five relocations with range checks, a flat binary, an ELF executable for `llvm-objdump`, and a bootable 160 KiB floppy with the 55AA signature checked.
- The runtime library (`rt/`, one section per routine): 32-bit divide, remainder, shifts and product; the 64-bit versions; `memcpy`, `memmove`, `memset`, `memcmp`, `bcmp`.
- The target description (`I8086*.td`): the machine instructions with their encoding in TSFlags (shared by the emitter, the decoder and the assembler) and the code generation instructions (`I8086InstrCodeGen.td`) that selection targets and `I8086MCInstLower.cpp` turns into them.

## How it was verified

The encoding table was written by hand from the structure of Intel's opcode map and kept apart from the implementation. The corpus check uses an independent source neither the table nor the backend derives from: bytes and disassembly of random instances captured from a real 8086 by the SingleStepTests project, cached by the hardware project. Writing the corpus check found the one bug the unit tables missed (the decoder did not count the fixed second byte of `aam`/`aad`).

Generated code is checked by running it. The emulator's 8088 core is itself verified against the physical-chip vectors and the RTL; the expected values are the host's integer arithmetic, so neither the compiler nor the core is its own oracle. Bring-up found these defects, all fixed: nested `EXTRACT_SUBREG` results without a type are silently dropped by TableGen (a `trunc` pattern never matched); a frame-index sentinel of -1 collided with the negative indices of incoming-argument objects (stack arguments read from address 0); the object writer and the text route chose different encodings for `mov ax, [abs]`; the carry of a wide add went through a branch because the combine that removes the flag-to-boolean round trip did not exist yet (a custom lowering sees the carry before its producer is lowered). The linker's own unit tests found that a reference to a symbol with a weak definition in its own object bound to that definition instead of the strong one in another object (fixed). The tests' own mistakes (an out-of-contract `zeroext` argument, a halted CPU kept across calls) were found the same way.

## Known gaps

- The 64-bit divide is a 64-step restoring loop and the 64-bit shifts are conditional constant shifts; the 32-bit divide uses `div` on the common paths. Division by zero: the unsigned 64-bit forms return all ones and the dividend (tested); the 32-bit forms reach the chip's own `div` and so raise its divide error (not tested); the signed forms are unspecified. Timing of the runtime routines is not measured.
- No `loop`/`jcxz`/`xlat` selection, no `in`/`out` intrinsics, no variadic functions, no `byval`, no interrupt attribute, no floating point.
- Code quality is first-order: a compare with a boolean result builds the value with a branch; a shift by eight of a value in a register pair does not reuse the same register; no peephole pass.
- No Rust target (`rust/i8086-none.json`) yet: milestone 7. The linker has no archives, no `-r`, no far relocations and no linker relaxation (`asm/README.md`); the image form is the 160 KiB floppy of the 5150 only (no `.COM`/`.EXE`).
- Intel manual page numbers for the table rows are still to be added; the corpus is the authority until then.
- `esc` (coprocessor escapes) and the `0F`/80186+ and undocumented opcodes are rejected on purpose.
- A label named `short`, `near` or `far` is read as a keyword.
