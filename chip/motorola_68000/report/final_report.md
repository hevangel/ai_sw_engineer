# MC68000 core — status report (2026-10-26/29)

## Result

**925,541 / 1,000,060 SingleStepTests vectors pass (92.6%).** The
target is zero mismatches (PR #269 states "do not merge until the
replay reaches zero mismatches"); the core is not mergeable yet.

The core also served as the executor for the llvm-m68k qualification
re-run (PR #263): the smoke image rebuilds byte-identical to the
recorded SHA-256, the mini -O0 image matches the Python oracle
byte-for-byte on both executors (retiring the old Defect 2), and the
mini -O1/-O2/-Os images diverge from the oracle in exactly slot 9 on
both executors — reproducing the LLVM flag-clobber miscompile
(Defect 1) on the pinned nightly with a trusted executor.

## What works

- Full canonical decode: all instruction groups, addressing modes,
  exception entry, privilege checks, USP/SSP banking.
- Suite-fitted semantics for: MOVEP (real encoding + direction),
  dynamic/static bit ops (kind map, Z-only flags), CHK (all
  destination registers, frame pc), MULU/MULS (gate + V always clear),
  NBCD (decimal correction formula + V window), ABCD/SBCD
  (nibble-serial decimal arithmetic + overflow windows), DBcc (branch
  base, GT condition), CMPM stepping, immediate-EA addressing,
  ADDQ/SUBQ/Scc/DBcc boundaries, MOVE SR,<ea> mode swap,
  ADDX/SUBX/ABCD/SBCD odd pre-decrement faults, address error frames
  (SSW labels, IIRD latching), BSR fault sequence (return push commits
  before the frame), TRAP vectors at 0x80+4n, two-step fetch-fault
  semantics.
- DIVU/DIVS word forms, fully passing (16,130 rows): remainder high
  word / quotient low word, N and Z from the quotient alone, V forced
  clear, C cleared, X preserved; overflow leaves the operands and all
  other flags untouched with V set; divide-by-zero stacks the
  instruction address and SR with N, Z and C cleared.

## Gaps (roughly 74,519 rows)

| Class | Rows (approx) | Blocker |
| --- | --- | --- |
| to-SR family (MOVEtoSR/ORItoSR/EORItoSR) + CMP/CMPA mode-7 rows | ~19,000 | suite's reference memory holds non-zero bytes the vector init list does not provide (operands like 0x7F/0x80 appear in the expected state but nowhere in init) |
| RTE | ~5,900 | odd-stack pops and format-fault frames need their own model |
| MOVEP.w / MOVEM.w / ORItoSR-EORItoSR-TRAP corner rows | ~7,000 | per-row corners, mostly flags and fault slots |
| BCHG/BSET/BCLR/CHK/DBcc/Scc/CMP.b-mode-4 residues | ~1,600 | small per-row corners |

## Verification commands

See `plans/testplan.md`. Everything documented in
`emulator/spec/singlesteptests.md` was derived from the suite, with
the fitting workflow recorded there.

## Not done yet

- Zero-mismatch gate (above).
- `cargo fmt` / `cargo clippy` pass and `tests/probe.rs` removal are
  deferred to the cleanup phase so the diagnostic harness survives
  until the row work is finished.
