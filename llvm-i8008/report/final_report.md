# llvm-i8008 final report

| Check | Result |
|---|---|
| MC encode and disassembly (lit) | pass: 227 instructions (`tests/mc/`) |
| Codegen (lit) | pass: alu, mem, call, branch (6/6 lit suites with mc) |
| Execution on the RTL | 4650/4650 cases (`exec_tests.py 30`) |
| Smoke program on the RTL | `tests/smoke/hello` prints its expected text |
| Titles built with this backend | Micral N boot ROM (241 / 256 B), MIC 01 monitor (2040 / 2048 B ROM), Shooting Stars (2458 B), SCELBAL (13826 B); all match their oracles on the RTL |

## Known gaps

- Register allocation leaves dead pair setups (`LEI`/`LDI` before `LHD; LLE`).
- Multi-byte carry chains compile poorly (four allocatable registers, no
  access to the carry flag from IR); SCELBAL keeps five such loops in asm.
- No secondary emulator yet; the RTL is the only execution oracle.

## Lessons

- Two escaped defects, both found by title oracles, not by the backend's
  own tests: the RTL's conditional return (an oracle defect,
  `failure_notes/2026-10-24-i8008-rtl-conditional-return.md`) and memset's
  value register (`failure_notes/2026-10-24-i8008-memset-value-register.md`).
- On a 7-level stack the call graph is a design input: the linker's depth
  check and its `RESET` points let a title keep its original control flow.
