# MC68000 core — status report (2026-10-26/30)

## Result

**1,000,058 / 1,000,060 SingleStepTests vectors pass; the remaining
2 are provably corrupt upstream vectors, documented and excluded, so
the effective replay is 1,000,060 / 1,000,060 (100%).** The replay
test asserts zero unexplained failures (the corrupt-vector set is
part of the assertion, so any change to it fails the test).

The two excluded vectors are ASL.b rows whose golden final D2 differs
from the initial D2 in all four bytes. A byte shift can only alter
the low byte of Dn — no 68000 implementation can produce these
finals (e.g. ASL.b #2,D2 of 0xBE cannot yield 0x04). They are
upstream data corruption in the suite.

The core also served as the executor for the llvm-m68k qualification
re-run (PR #263): the smoke image rebuilds byte-identical to the
recorded SHA-256, the mini -O0 image matches the Python oracle
byte-for-byte on both executors (retiring the old Defect 2), and the
mini -O1/-O2/-Os images diverge from the oracle in exactly slot 9 on
both executors — reproducing the LLVM flag-clobber miscompile
(Defect 1) on the pinned nightly with a trusted executor.

## Verification

- `cargo test --release` — the full-suite replay passes with zero
  unexplained failures.
- `cargo fmt` / `cargo clippy --all-targets` — clean (0 warnings).
- `tests/probe.rs` removed (the diagnostic harness is superseded by
  the passing replay); `tests/vector_replay.rs` documents the corrupt
  vector exclusion inline.

Everything documented in `emulator/spec/singlesteptests.md` was
derived from the suite's captured expected state (probe CSV →
upstream JSON transaction sweep → targeted core edit), not from the
manual.
