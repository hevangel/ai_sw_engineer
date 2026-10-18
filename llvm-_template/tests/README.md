# llvm-[CHIP] tests

- `codegen/` — `.ll` inputs with `; RUN: llc -march=<chip> %s -o - | FileCheck %s`
- `mc/` — `.s` inputs with `; RUN: llvm-mc -triple=<chip> -show-encoding %s | FileCheck %s`, one per opcode in `spec/isa.md`
- `smoke/` — small Rust/asm programs run on the simulator with expected final state
- `lit.cfg.py` — add when the first test lands
