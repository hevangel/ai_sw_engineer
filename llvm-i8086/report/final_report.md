# llvm-i8086 report

Status (2026-10-25): milestone 2, the machine-code layer, is done; code generation (milestone 3) has not started. There is no `llc` for this target yet, no runtime library and no Rust target that builds.

| Check | Result |
|---|---|
| Build | `scripts/build_llvm.sh` builds `llvm-mc`, `llvm-objdump`, `llvm-readobj`, `llvm-tblgen`, `FileCheck`, `not`, `count` with the experimental `I8086` target (about 8 minutes cold with ccache; a target-only change rebuilds in under a minute) |
| MC lit suite | 7 of 7 files pass: `encodings.s` (423 rows from the manual's opcode structure, 36 branch encodings with fixups), `disasm.txt`, `disasm-invalid.txt`, `relax.s`, `range.s`, `fixups.s`, `errors.s` |
| Physical-chip corpus (`tests/mc/vector_check.py`) | 27,900 instruction instances (100 per opcode file, seed 11, the final build): all decode with MartyPC's mnemonic and no leftover bytes; 24,333 re-assemble byte for byte, 667 to an alternative encoding that disassembles identically, 2,900 relative branches checked for decoding only; 0 problems. An earlier 33,480-instance run (seed 7) was equally clean |
| Smoke programs, codegen tests, titles | not applicable yet |

## What exists

- `llvm-mc -triple=i8086`: Intel-syntax assembler and disassembler for the whole 8086 integer instruction set (no 80186+ and no undocumented opcodes), memory operands in all eight base/index forms with 8- and 16-bit displacements, segment overrides, prefixes, string instructions, far `jmp`/`call`, port I/O.
- Branch relaxation: bare `jmp` grows from `EB` to `E9`; bare `jcc` grows to the five-byte inverted-jump-over-near-jump; `loop`/`jcxz` and `short` forms do not relax and diagnose out-of-range targets.
- ELF32 objects with `e_machine` 0x8086 and the relocation kinds of `spec/abi.md`, read by `llvm-objdump` and `llvm-readobj`.
- The TableGen description (`I8086.td` and friends) with each instruction's encoding in TSFlags, shared by the emitter, the decoder and the assembler; a stub `LLVMInitializeI8086Target`.

## How it was verified

The encoding table was written by hand from the structure of Intel's opcode map and kept apart from the implementation. The corpus check uses an independent source neither the table nor the backend derives from: bytes and disassembly of random instruction instances captured from a real 8086 by the SingleStepTests project, cached by the hardware project. Writing the corpus check found the one bug the unit tables missed (the decoder did not count the fixed second byte of `aam`/`aad`); a negative control (corrupting one expected encoding) fails as it should.

## Known gaps

- Intel manual page numbers for the table rows are still to be added; the corpus is the authority until then.
- `esc` (coprocessor escapes) and the `0F`/80186+ and undocumented opcodes are rejected on purpose.
- A label named `short`, `near` or `far` is read as a keyword.
- Milestone 3 onward: selection, frames, calls, the runtime library, the linker and packager, the Rust target.
