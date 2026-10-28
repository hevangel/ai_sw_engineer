# Implementation plan — MC6809 Rust core

Issue: ai_sw_engineer#265. Deliverable: `emulator/`, a `no_std`
bus-cycle-accurate MC6809 library.

| Step | Scope | Status |
| --- | --- | --- |
| M1 | Fixtures: commit the reyco2000 functional/interrupt/cycle programs, listings, license and sources under `oracle/reyco2000/` | done |
| M2 | Core skeleton: `Bus` trait, `Mc6809` state, `reset`, `tick` = one clock, micro-op queue with data-sheet cycle totals | done |
| M3 | Semantics: pages 1/2/3, all addressing modes (indexed postbyte + indirect), TFR/EXG, PSH/PUL stacks, DAA/MUL/ABX/SEX, CWAI/SYNC, SWI/SWI2/SWI3, RTI, FIRQ/IRQ/NMI frames — ported from the ai_hw_engineer RTL | done |
| M4 | Harness: DEV test device ($FF00-$FF0F) ported from the RTL testbench, S-record loader, suite runner with self-loop failure detection | done |
| M5 | Validation: reyco2000 functional, interrupt and cycle suites pass unmodified (exit 0, zero cycle mismatches) | done |
| M6 | Report + spec/assumption ledger; RTL timing deviations documented | done |

Deferred: none.
