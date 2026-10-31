# llvm-mos6502 implementation plan

Milestones (each ends with passing tests):

1. ISA + ABI specs written; Rust target JSON drafted.
2. MC layer: register info, instruction definitions, encoder; assemble every opcode (`tests/mc/`).
3. Instruction selection for integer add/sub/logic/load/store/branch at the native width (`tests/codegen/`).
4. Legalization of wider types; runtime helpers; calls and stack frames.
5. Linker + image packaging; `scripts/build_image.sh` contract satisfied.
6. `core` builds via `-Zbuild-std=core`; smoke programs run on the simulator (`tests/smoke/`).
7. First real title compiled and run.
