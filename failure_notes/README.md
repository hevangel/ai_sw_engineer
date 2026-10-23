# Failure notes

Post-mortems for bugs that **escaped verification**: found by the original program's behavior, a second emulator, a reviewer or real use — not by the title's or backend's own tests.

File name: `YYYY-MM-DD-<area>-<short-name>.md`.

## Template

```markdown
# <title>

- Area: system/<system>/<software> | llvm-<chip> | platform
- Found by: <external oracle>

## What broke
## Root cause — and why our tests missed it
## Fix and re-verification
## Prevention (rule/test added; link the AGENTS.md change if any)
```

Blame-free: the question is which step let a shared assumption pass as verified. Note in particular whether the bug was in the Rust reimplementation, the LLVM backend, the platform runtime, or the simulator.

## Index

- `2026-10-18-busicom_141pf-clear-prints-zero.md` — `C` printed a blank row
  instead of the cleared value `0`; caught by the original ROM's port trace
  while adding `tests/port_trace.rs`.
- `2026-10-18-busicom_141pf-partial-tape.md` — the reimplementation printed a
  thinner tape than the ROM (missing `×`/`÷` and `M+`/`M-` operand echoes, a
  single symbol column instead of two, and an incomplete lamp nibble); caught by
  the original ROM's port trace and fixed by making `tests/port_trace.rs` an
  exact full-tape comparison (spec A6–A9).
- `2026-10-22-llvm-i4004-signed-compares-as-unsigned.md` — `icmp slt/sle/sgt/sge`
  lowered as unsigned; caught by external review (issue #18), fixed with sign-
  flipped borrow compares and `scripts/exec_cmp.py` (all predicates on chips).
- `2026-10-22-llvm-i4004-frame-slots-alias.md` — the frame offset was passed as
  `addExternalSymbol`'s target-flags argument, so every spill slot aliased at
  `__frame_<fn>+0`; caught by `exec_cmp.py` on chips.
- `2026-10-22-llvm-i4004-packer-relaxation.md` — page relaxation in
  `i4004_pack.py` mis-placed relaxed branches in five ways (unshifted
  local-label addends, section-relative skip operands, a skip target off the
  page at 0xFC, a JCN at 0xFE/0xFF landing on the next page, stale skip bytes
  after a later splice); caught by `exec_num.py` running `asm/num.s` on chips.
- `2026-10-22-busicom_141pf-div-scale-shortfall.md` — fixed-point division zero-
  filled the last `sb − sa` places when the divisor had more decimals (1 ÷ 0.003
  → 333.00); inherited from M2 and reproduced by every twin; caught in review.
- `2026-10-22-busicom_141pf-target-only-driver-defects.md` — printer data
  polarity, held-key repeat, time-driven drum and a 7-deep call graph, all
  invisible to the host board model; caught by reading the simulator harnesses
  before the first flat target run.
- `2026-10-22-llvm-i4004-bool-return-in-wrong-pair.md` — a `zeroext i1` return
  was widened to `i16` and returned in P2:P3 while the caller read P1 (no
  `getTypeForExtReturn` override); caught by the first 42-example run on chips.
- `2026-10-22-busicom_141pf-drum-phase.md` — `__pr_print` keyed sector 0 on the
  TEST level, which external/chips and the RTL present with opposite JCN
  phase, so rows printed one sector early on the RTL; caught by the first RTL
  run (chips was 42/42).
