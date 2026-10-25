# llvm-i8008 implementation plan

| # | Milestone | Status |
|---|---|---|
| 1 | ISA + ABI specs; Rust route | done |
| 2 | MC layer: registers, instructions, encoder, assembler, disassembler; every opcode encodes | done (`tests/mc/`) |
| 3 | Instruction selection for i8/i16 arithmetic, logic, loads, stores, branches | done (`tests/codegen/`) |
| 4 | Legalization of wider types; `rt/` helpers; calls, static frames, memory arguments | done (`exec_tests.py`) |
| 5 | Linker: layouts, GC, frames by overlay, call depth, relocations | done (`asm/i8008_link.py`) |
| 6 | Rust `core` via build-std; smoke program on the RTL | done (`tests/smoke/hello`) |
| 7 | Titles: Micral N boot ROM, monitor, Shooting Stars, SCELBAL | done |
| 8 | Size: H:L and A tracking, IPRA, `__leaf_` convention, data-page mode, byte pairs, aligned-data known bits, page-indexed accesses, branch-free compares, best-fit placement | done |

Next: better register allocation around H:L (an `LEI %lo(x)` before a pair
move is often dead); a branch on a compare whose value is also kept still
compares twice.
