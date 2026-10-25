# llvm-i8086 test plan

| Suite | Tool | Oracle | Location |
|---|---|---|---|
| MC encode | `llvm-mc -show-encoding` + FileCheck | the manual's opcode table, written out by hand (`gen_encodings.js`), 188 rows | `tests/mc/encodings.s` |
| MC decode | `llvm-mc --disassemble` + FileCheck | hand-worked | `tests/mc/disasm*.txt` |
| Relaxation, relocations, diagnostics | `llvm-mc`, `llvm-objdump`, `llvm-readobj` | hand-worked from `spec/abi.md` | `tests/mc/{relax,range,fixups,errors}.s` |
| MC vs the chip | `tests/mc/vector_check.py` | SingleStepTests/8086 (physical Intel P80C86A-2): bytes and MartyPC disassembly of random instances of every opcode | `tests/mc/` |
| Codegen | `llc` + FileCheck | hand-checked expected asm | `tests/codegen/` (milestone 3) |
| Execution | the `system/ibm_pc` emulator, then the RTL | expected register/memory state | `tests/smoke/` (milestone 6) |
| Rust end-to-end | `rustc` → image → emulator | known output | `tests/smoke/` |

Execution tests run on `system/ibm_pc/emulator` (itself verified against the physical-chip vectors and the RTL) and on the ai_hw_engineer RTL.

Every instruction form in `I8086InstrInfo.td` is exercised by the corpus check
(except ESC and the undocumented encodings, which the backend rejects by design)
and every assembler-visible form has an `encodings.s` row.
