# llvm-i8086 implementation plan

Milestones (each ends with passing tests):

1. ISA + ABI specs written; Rust target JSON drafted. **Drafted** (this change); the ISA table still owes the row-by-row check against the Intel manual, which is the first task of milestone 2.
2. MC layer: register info, instruction definitions, encoder; assemble every opcode (`tests/mc/`), each checked against the manufacturer opcode table; reject every 186+ opcode. **Done 2026-10-25**: `llvm-mc`/`llvm-objdump`/`llvm-readobj` for the i8086 target (assembler, disassembler, relaxation, ELF writer with EM 0x8086 and five relocation types); 423 table rows, 36 branch encodings and the physical-chip corpus all pass (`report/final_report.md`). The instruction forms live in `I8086InstrInfo.td`, their encoding in TSFlags, interpreted by `I8086MCCodeEmitter.cpp` and `I8086Disassembler.cpp`.
3. (next) Instruction selection for integer add/sub/logic/load/store/branch at the native width (`tests/codegen/`); the assembler already relaxes out-of-range `Jcc`.
4. Legalization of wider types; runtime helpers; calls and stack frames per `spec/abi.md`.
5. Linker + image packaging: flat binary at the platform load address, boot-sector image with the 55AAh signature; `scripts/build_image.sh` contract satisfied.
6. `core` builds via `-Zbuild-std=core`; smoke programs run on the simulator (`tests/smoke/`): first a boot sector that writes to CGA memory, observed on the `system/ibm_pc` emulator and on the `ai_hw_engineer` RTL.
7. First real title compiled and run.

Whole-program size gate from the first function that lowers (AGENTS): link and measure against the RAM budget in `system/ibm_pc/platform/memory_map.md`.

## First title: open decision

The title is not chosen. Candidates, to be checked for license and provenance before any is committed to:

- A boot-sector program of our own (no copyright issue; useful as the smoke test and as a proof of the boot path).
- Software whose source is openly licensed, for example the MS-DOS 1.25 / 2.0 source Microsoft released under the MIT license (to be confirmed and cited in `original/provenance.md`), or other freely redistributable early PC utilities.
- IBM's own BIOS or Cassette BASIC: the oracle can run them (the hardware project boots the unchanged ROMs), but they are copyrighted, so only hashes and provenance may be committed.

## Companion work in `system/ibm_pc`

The emulator (`system/ibm_pc/emulator/`) is planned in parallel: 8088 core and peripherals written here, cross-checked against the RTL and MartyPC; see its `spec/spec.md` and `plans/testplan.md`.
