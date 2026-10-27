# Final report — MC6809 Rust core (ai_sw_engineer#265)

Date: 2026-10-26. Status: **implemented and validated**.

## Deliverable

`emulator/` — `no_std` MC6809 library (`motorola_6809` crate), cycle-driven:
one `tick` per bus clock; instructions decode into micro-op sequences whose
totals match the data sheet; IRQ/FIRQ level-sampled and NMI edge-latched at
instruction boundaries and in CWAI/SYNC wait states; the DEV test device of
the reyco2000 suite is ported in the test harness.

## Verification (commands + results)

Host toolchain: rustc 1.99.1 (`RUSTUP_TOOLCHAIN=1.99.1`), Windows host.

- `cargo test --manifest-path chip/motorola_6809/emulator/Cargo.toml`
  - `functional_test` **PASSED** — the unmodified reyco2000 functional
    suite (all documented instructions, every defined CC bit, DAA/MUL
    vectors, frames), exit code 0 (787,857 clocks).
  - `interrupt_test` **PASSED** — IRQ/FIRQ/NMI masking, priority, level
    vs edge, frames and stacked CC bits, NMI disarm until LDS, nested
    IRQ in a FIRQ handler, CWAI and SYNC (10,347 clocks).
  - `cycle_test` **PASSED** — the reyco2000 cycle-count suite with
    **zero mismatches** (exit 0, 6,310 clocks): every instruction form's
    data-sheet cycle count, all indexed modes' extra cycles, indirect
    modes, branches taken/not taken, stack instructions, SWI/RTI.
- `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings`
  clean.

## Timing comparison with the ai_hw_engineer RTL

The RTL passes the reyco2000 functional (533,184 clocks) and interrupt
(7,415 clocks) suites but is 1-2 cycles short of the data sheet on
several direct/extended/indexed forms, and its own `run_sim.sh` never
runs the cycle suite — a direct RTL run of it times out at the SYNC
timing check (pc $10C1). This core implements the data-sheet totals the
cycle suite verifies. The semantic contract (flag formulas, frames,
masks, postbyte table, assumptions) is ported from the RTL and confirmed
by both functional suites passing. Worth wiring the cycle suite into
`design/motorola_6809` CI under
[ai_engineer#46](https://github.com/hevangel/ai_engineer/issues/46).

## Bugs caught by the suites during development

- IMM16 operands were fetched as opcodes (no immediate ea/pc advance).
- Page-2/3 prefix ($10/$11) was never latched — page-2/3 opcodes decoded
  as page-3 NOPs.
- JSR baked its return address at decode time, before the operand bytes
  were consumed (wrong return PC for direct/extended/indexed JSR); the
  return push is now a dedicated micro-op at EA-complete time.
- Extended-indirect ([n16]) postbyte check compared the raw postbyte
  (0x9F) against 0x1F — the pointer word was added to the index register
  instead of becoming the pointer.
- LBRA was padded as a taken conditional long branch (6 instead of 5).
- PSHU/PULU base was assumed 7; the suite's measurements give 5 + bytes
  (same as PSHS/PULS).
- Harness: IRQ/FIRQ line levels were not forwarded from the DEV device
  to the core (only NMI pulses were); the RTL testbench wires them
  continuously.

## Limits

- Pad placement inside multi-cycle instructions is approximate (totals
  are exact and suite-verified); the suite measures instruction totals,
  not intra-instruction bus phase placement.
- The DEV device, cycle latches and exit conventions are harness-side,
  mirroring the RTL testbench.
- Undocumented opcodes follow the RTL's NOP conventions (pages 1/2/3).
