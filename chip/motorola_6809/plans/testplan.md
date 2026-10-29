# Test plan — MC6809 Rust core

All tests run on the host: `cargo test --manifest-path
chip/motorola_6809/emulator/Cargo.toml`.

1. **Functional suite** (`tests/reyco.rs::functional_test`) — the
   unmodified reyco2000 `6809_functional_test`: every documented
   instruction in every addressing mode, results and every defined
   condition-code bit, DAA over 1024 inputs, stack frames and side
   effects. Passes when the program writes 0 to DEV_EXIT; any failing
   check self-loops (the run loop detects it and reports the PC).
2. **Interrupt suite** (`tests/reyco.rs::interrupt_test`) — IRQ, FIRQ,
   NMI: masking, priority, level vs edge sensing, frames and stacked CC
   bits, NMI disarmed until the first LDS, IRQ nesting inside a FIRQ
   handler, CWAI (both flavors + already-pending), SYNC (masked,
   unmasked, already-active), via the DEV device model ported from the
   RTL testbench.
3. **Cycle-count suite** (`tests/reyco.rs::cycle_test`) — every
   instruction form's data-sheet cycle count measured through the DEV
   cycle-counter latches, including every indexed mode's extra cycles,
   indexed indirect, branches taken/not taken, stack instructions and
   SWI/RTI. Zero mismatches required (the exit code is the mismatch
   count).

The suite programs are committed under `../oracle/reyco2000/` with their
GPL-3.0 license and the upstream sources, so failures can be mapped to
source lines via the committed listings.
