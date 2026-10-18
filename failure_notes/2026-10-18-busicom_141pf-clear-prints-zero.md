# `C` (clear) printed a blank row instead of `0`

- Area: system/busicom_141pf/firmware
- Found by: the original 1280-byte ROM's own port trace (`oracle/rom_port_trace.txt`), while adding `tests/port_trace.rs`

## What broke

`Calculator::clear` (the `C` key) printed a row with the `C` symbol and **no
digits** (`Row::blank(Symbol::Clear)`). The original ROM prints the cleared
value `0` with the `C` symbol: every example's setup echo begins

```
ROW|0|C| |false      (C)
ROW|0|C|M|false      (CM)
ROW|0|C| |false      (C)
```

so the `C` rows carry `0`, not blank. Our drivers therefore produced a `C` row
whose numeric columns were all blank where the ROM struck the `0` digit.

## Root cause — and why our tests missed it

The bug was in the **Rust reimplementation** (the arithmetic core), not the
drivers, backend, or board model — the drivers faithfully rendered the blank
`Row` the core handed them.

It survived because the title's sanctioned oracle, `oracle/manual_examples.json`
(recovered-ROM profile), checks each step's tape as a **suffix** and never lists
a `C`-symbol row in any step's expectation (a search finds zero `C` rows in the
step tapes). The `C` key only ever appears in the per-example *setup* echo
(`CE C CM C`), which the replay harness runs but does not assert. So the printed
form of `C` was an unverified assumption — recorded honestly as spec
**ASSUMPTION A3** ("`C` prints … no digits"), with its validation listed as the
then-unfinished "recapture `C` on the original ROM". The port trace is that
recapture, and it disproved the guess.

## Fix and re-verification

`Calculator::clear` now prints `Row::number(&Num::ZERO, Symbol::Clear)`, which
renders the single digit `0` (a zero `Num` has `width() == scale + 1 == 1`) with
the `C` symbol.

```rust
// was: out.print(&Row::blank(Symbol::Clear));
out.print(&Row::number(&Num::ZERO, Symbol::Clear));
```

Re-verified in the Docker toolchain container (`scripts/docker_run.sh`):

- `tests/manual_examples.rs` and `tests/port_level.rs` still pass 42/42 (the
  change does not touch any checked suffix).
- `tests/port_trace.rs` (new) now matches the ROM's `C` rows; the C-prints-zero
  class of divergences is gone.

## Prevention

- Added `tests/port_trace.rs`, which replays all 42 examples through the drivers
  and compares the produced port events against the independent original-ROM
  trace `oracle/rom_port_trace.txt` — the first test that reads that trace. It
  checks the **full** per-step row stream (not just a suffix), so an unverified
  printed-row assumption like A3 can no longer pass silently.
- Spec A3 updated from a guess to a trace-validated fact.
- The remaining assumptions the same trace exposed (A6 operand echo, A7 memory-
  key echo, A8 two symbol columns, A9 negative/undocumented lamp bits) are
  recorded in `spec/spec.md` and explicitly gated in `tests/port_trace.rs`
  rather than silently ignored — so they are now tracked debt, not hidden gaps.
