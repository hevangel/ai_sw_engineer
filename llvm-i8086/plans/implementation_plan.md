# llvm-i8086 implementation plan

Milestones (each ends with passing tests):

1. ISA + ABI specs written; Rust target JSON drafted. **Drafted** (this change); the ISA table still owes the row-by-row check against the Intel manual, which is the first task of milestone 2.
2. MC layer: register info, instruction definitions, encoder; assemble every opcode (`tests/mc/`), each checked against the manufacturer opcode table; reject every 186+ opcode. **Done 2026-10-25**: `llvm-mc`/`llvm-objdump`/`llvm-readobj` for the i8086 target (assembler, disassembler, relaxation, ELF writer with EM 0x8086 and five relocation types); 423 table rows, 36 branch encodings and the physical-chip corpus all pass (`report/final_report.md`). The instruction forms live in `I8086InstrInfo.td`, their encoding in TSFlags, interpreted by `I8086MCCodeEmitter.cpp` and `I8086Disassembler.cpp`.
3. Instruction selection for integer code at 8 and 16 bits: ALU, loads and stores with all the 8086 addressing modes, compares and branches, selects, shifts, extensions, calls and stack frames. **Done 2026-10-25**: `llc -mtriple=i8086` produces assembly and ELF objects (`spec/codegen.md`); 5 FileCheck files in `tests/codegen/`, and 7 execution programs in `tests/run/` run on the 8088 core and checked against host arithmetic (`report/final_report.md`). Calls and frames (`abi.md` D3, D4) came with it.
4. Native `mul`/`div`, add-with-carry and `rep movs`/`stos`. **Done 2026-10-25** (`spec/codegen.md`): 16-bit multiply and divide on AX/DX, 32-bit products from 16-bit full products, `add`/`adc`/`sub`/`sbb` chains for 32- and 64-bit values, block copies and fills. Wider types legalize into 16-bit pieces through the generic legalizer.
5. The runtime library in `rt/`. **Done 2026-10-25** (`rt/README.md`): 32-bit divide, remainder and shifts in assembly, `memcpy`/`memmove`/`memset`/`memcmp`/`bcmp`, and the 64-bit multiply, divide, remainder and shifts in LLVM IR; one section per routine. Variadic calls and `byval` arguments came later (milestone 9).
6. Linker + image packaging. **Done 2026-10-25** (`asm/README.md`): `i8086-ld` places sections into the regions of the platform linker script, collects unused sections, applies the five relocation types, checks region sizes, and writes a flat binary, an ELF executable for `llvm-objdump`, and a bootable 160 KiB floppy with the 55AA signature; `scripts/build_image.sh` drives it from `.o`/`.s`/`.ll` inputs. A two-stage boot program is linked, packed, and booted through the original BIOS in `system/ibm_pc/emulator/tests/i8086_boot.rs`.
7. `core` builds and Rust programs run. **Done 2026-10-26** (`rust/README.md`): rustc builds `core` and a crate as msp430 (same data layout as D1), `scripts/build_rust.sh` retargets the IR and runs our `llc`; `tests/smoke/hello_rs` boots through the original BIOS and prints what the host computes, `tests/smoke/stress_rs` (one source, run on both) agrees with the host at five optimisation levels. The RTL boot comparison is done (`system/ibm_pc/emulator/tests/boot_oracle.rs`: same screen and same final CPU state on both images). The real title (milestone 8) and variadic/`byval` (milestone 9) followed; a rustc with an i8086 target is still to do.
8. First real title compiled and run. **Done 2026-10-26**: the Rust `CHKDSK.COM` of `system/ibm_pc/chkdsk` is identical to PC DOS 1.00's original on 24 disks, on the Rust emulator and on the RTL.
9. Variadic functions and `byval` arguments. **Done 2026-10-27** (`spec/abi.md` D3a): all arguments of a variadic call on the stack, `va_list` a slot pointer, `byval` copied by pushes; FileCheck in `tests/codegen/vararg.ll` and execution tests against host arithmetic in `tests/run/vararg.ll`.

Whole-program size gate from the first function that lowers (AGENTS): link and measure against the RAM budget in `system/ibm_pc/platform/memory_map.md`.

## First title: open decision

The title is not chosen. Candidates, to be checked for license and provenance before any is committed to:

- A boot-sector program of our own (no copyright issue; useful as the smoke test and as a proof of the boot path).
- Software whose source is openly licensed, for example the MS-DOS 1.25 / 2.0 source Microsoft released under the MIT license (to be confirmed and cited in `original/provenance.md`), or other freely redistributable early PC utilities.
- IBM's own BIOS or Cassette BASIC: the oracle can run them (the hardware project boots the unchanged ROMs), but they are copyrighted, so only hashes and provenance may be committed.

## Companion work in `system/ibm_pc`

The emulator (`system/ibm_pc/emulator/`) is planned in parallel: 8088 core and peripherals written here, cross-checked against the RTL and MartyPC; see its `spec/spec.md` and `plans/testplan.md`.
