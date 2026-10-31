# MC68000 core — test plan

## Oracle replay (primary)

`RUSTUP_TOOLCHAIN=1.99.1 M68K_VECTOR_DIR=<compiled suite> cargo test
--release --test vector_replay`

- Passes when every vector reproduces the captured registers, PC, SR
  and memory exactly; it prints the per-file failure counts on
  failure.
- Suite source: `ai_hw_engineer/design/motorola_68000/work/sim`
  (compiled streams made by `make_68000_vectors.py`).

## Row probe (diagnostic, not a gate)

`PROBE_FILE=<suite file> [PROBE_ROW=<n>] cargo test --release --test
probe` — dumps every failing row of one file (full initial state,
suite-final state, all memory diffs) to stdout and
`%TEMP%/m68k_probe.csv`. `tests/probe.rs` is scratch tooling and is
removed before merge.

## Unit checks

`cargo test --release` — lib unit tests plus the replay gate. The
replay is expected to fail (non-zero mismatches) until phase 3 of
`implementation_plan.md` completes.

## Regression rules

- Any decode or flag change re-runs the full replay; a change that
  fixes one file must not regress another (watch the per-file counts).
- Frame-layout changes are checked against at least one row from each
  faulting family (MOVE, Bcc/DBcc, BSR, RTS/RTR, pre-decrement).
