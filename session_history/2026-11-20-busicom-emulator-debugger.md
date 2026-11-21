# 2026-11-20 - Busicom 141-PF emulator: instruction-level debugger

Branch `system/busicom-141pf-emulator-debug`, issue hevangel/ai_sw_engineer#23.
Single session; the model was selected dynamically by the host (Auto).

## Outcome

- `busicom_emulator::debug` and a `debug` CLI subcommand: single step, run,
  breakpoints (stop after the instruction at the address ran), CPU state
  before each instruction, RAM/port inspection. Closes the "debugging
  facilities" part of acceptance item 4.
- `advance` now runs through one shared `step_cycle`; the release trace of
  the 42 manual examples is byte-identical before and after, and the
  ignored original-ROM replay test passes.

## Decisions

- `external/chips` keeps CPU registers private and is not edited, so state
  is read from the model's per-instruction `log::debug!` record through a
  thread-local probe logger. Consequence: breakpoints stop after the
  instruction at the address, and the probe slows stepping (off by default).
- Test expectations come from MCS-4 data sheet semantics (hand-assembled
  programs), not from emulator output.
- Clippy 1.99 flags the const `thread_local!` items as non-const; allowed at
  module level with a comment.

## Validation

`cargo test` (4 new), original-ROM manual-example replay (ROM SHA-256
`c6af325d...ceb7`), `fmt --check`, `clippy -D warnings`, release build. The
1000-program random replay was not rerun.

## Not measured

Kiro does not expose per-model token totals; none are recorded.

## Open (issue #23)

Emulator inventory, machine-contract spec shared with #65, independent
emulator comparison, graphical panel, XL RAM/banked ROM.
