# Implementation plan — MC6800 Rust core

Issue: ai_sw_engineer#264. Deliverable: `emulator/`, a `no_std`
bus-cycle-accurate MC6800 library.

| Step | Scope | Status |
| --- | --- | --- |
| M1 | Fixtures: commit the ai_hw_engineer oracle artifacts (full-ISA vectors + memory + triggers; 50 assembled third-party programs with ISS-computed windows) under `oracle/` with provenance | done |
| M2 | Core skeleton: `Bus` trait, `Mc6800` state, `reset`, `tick` = one clock, micro-op queue with exact documented cycle totals | done |
| M3 | Semantics: decode the 197 documented opcodes (oracle NOP convention for the rest), ALU/flags/DAA/RMW/16-bit/inherent, branches, WAI/SWI/RTI, IRQ/NMI service | done |
| M4 | Validation: full-ISA vector replay (boundaries, triggers, memory) and the 50-program third-party suite in `emulator/tests/` | done |
| M5 | Report + spec/assumption ledger; flag oracle-level deviations found along the way | done |

Deferred: none. The core is a library; a runner binary is not needed for
the deliverable (tests are the runner).
