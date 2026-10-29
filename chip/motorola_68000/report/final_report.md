# MC68000 core — status report (2026-10-26/29)

## Result

**970,952 / 1,000,060 SingleStepTests vectors pass (97.1%).** The
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
- Suite-fitted semantics for: MOVEP (real encoding + direction, high
  word preserved on .w loads), dynamic/static bit ops (kind map,
  Z-only flags), CHK (all destination registers, frame pc),
  MULU/MULS (gate + V always clear), NBCD (decimal correction formula
  + V window), ABCD/SBCD (nibble-serial decimal arithmetic + overflow
  windows), DBcc (branch base, GT condition), CMPM (opmode 5/6 gate),
  immediate-EA addressing, ADDQ/SUBQ/Scc/DBcc boundaries,
  MOVE SR,<ea> canonical stepping, MOVE <ea>,SR decode (0x46C0-FF)
  with the golden's 0xA71F SR mask, ORI/EORI-to-SR, ADDX/SUBX/ABCD/
  SBCD odd pre-decrement faults, address error frames (SSW labels,
  IIRD latching), BSR fault sequence (return push commits before the
  frame), TRAP vectors at 0x80+4n, two-step fetch-fault semantics,
  DIVU/DIVS word forms (fully passing), RTE (odd-PC frame labels,
  popped I-mask kept), MOVEM (word loads sign-extend into the whole
  register, pre-dec forms step by the transfer size and roll the
  register back on a parity fault), trace sampling at instruction
  start, exception entry keeping the interrupt mask.

## Gaps (roughly 29,108 rows)

| Class | Rows (approx) | Blocker |
| --- | --- | --- |
| ADD/ADDX/SUBX/OR families | ~13,400 | per-row golden quirks not yet swept (likely fault frames and flag corners like the CMP family) |
| MOVEM.w/l residue | ~1,860 | pre-dec store slot/register order and fault-frame corners |
| MOVE.l / LINK | ~2,980 | unswept |
| ABCD / NBCD | ~950 | per-row corners |
| Bcc / DBcc | ~209 | small residues |

## Verification commands

See `plans/testplan.md`. Everything documented in
`emulator/spec/singlesteptests.md` was derived from the suite, with
the fitting workflow recorded there.

## Not done yet

- Zero-mismatch gate (above).
- `cargo fmt` / `cargo clippy` pass and `tests/probe.rs` removal are
  deferred to the cleanup phase so the diagnostic harness survives
  until the row work is finished.
