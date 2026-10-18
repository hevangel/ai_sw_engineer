# llvm-[CHIP] test plan

| Suite | Tool | Oracle | Location |
|---|---|---|---|
| MC encode | `llvm-mc -show-encoding` + FileCheck | manufacturer opcode table (`spec/isa.md`) | `tests/mc/` |
| Codegen | `llc` + FileCheck | hand-checked expected asm | `tests/codegen/` |
| Execution | simulator | expected register/memory state | `tests/smoke/` |
| Rust end-to-end | `rustc` → image → simulator | known output | `tests/smoke/` |

Execution tests run on the ai_hw_engineer RTL/ISS where one exists and on a third-party emulator otherwise.
