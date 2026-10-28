# The random calculator corpus found five firmware-core behaviours the original ROM contradicts

- Area: system/busicom_141pf/firmware (`bcd.rs`, `lib.rs` host core), found
  while building the emulator's random verification corpus
  (`platform/scripts/gen_random_tests.py`,
  `oracle/random_calculator_tests.json`)
- Found by: the seeded random corpus replayed on the **original ROM** through
  the system emulator (`emulator/tests/random_replay.rs`, CLI +
  `compare_oracle.py`) and scored against expected tapes computed by the
  firmware host core. 88 of 1000 examples diverged on the first unconstrained
  corpus (issue #23 work, 2026-10-25).

## What broke

The host core agrees with the ROM on all 42 manual examples and the full
port trace, but five behaviours diverge on inputs those oracles never
exercise. Representative rows (core expectation vs original ROM):

1. **FL result width.** The core fills floating division/multiplication
   results to 13 significant digits; the ROM reaches 14 when the digit
   stream continues: `7.2 ÷ 0.7` → core `1.028571428571`, ROM
   `1.0285714285714` (R0030). The anchors disagree with each other about the
   rule: 3-1 (`6.6666666666666`) is 14 digits but 3-4 (`25.98225602027`) and
   4-2 (`66.66666666666`) are 13 — the ROM's exact digit-emission rule is not
   yet pinned. The 42-example suite cannot see this: 13-significant
   truncation happens to match 3-4/4-2, and 3-1 is produced correctly by
   whichever path generates `6.6666666666666` today.
2. **False overflow at width 14.** The core overflowed results the ROM
   printed: a FL quotient `270945.46881173` (R0165), a 14-wide exact chain
   product at 5/4 (`2421511.357372 × 645.4`, R0917), and a fourth power of a
   4-fraction-digit entry (R0580). The capacity anchors (15-1..16-4) only
   cover 14-wide single-step results, not chained or fractional-width-14
   cases.
3. **Zero results lose their scale.** For a zero total, subtotal or product
   the core prints `0` where the ROM keeps the decimal position: `0.00`
   (R0069), `0.0` (R0453), `0.00000` (R0686).
4. **Square root of a recalled radicand.** After a floating-precision result
   the ROM prints the radicand as-is (`304.66666666666`) while the core
   rescales it to DP (`304.66`); the root's last digit diverges as a
   consequence (`17.454703282114` vs `17.454703282112`, R0062). The anchors
   (11-2, 11-3) only recall DP-scaled values, where both agree.
5. **`M±` alignment is inconsistent in the core.** For `49991.18 M+` at DP 1
   the core prints the truncated echo `49991.1` but stores the 5/4-rounded
   `49991.2` into memory; the ROM truncates both (A1) (R0256).

## Root cause — and why our tests missed it

The core's arithmetic was validated against the 42 manual examples, the
committed ROM port trace, its own assembly twin and the RTL. Every one of
those anchors the same 42 input shapes. The five behaviours live strictly
outside those shapes: widths past 13 digits, zero results, recalled
floating-width radicands, and DP-overflowing memory entries. A differential
test suite is only as independent as its narrowest shared oracle — the
random corpus against the original ROM is the first check in this
repository that compares core and ROM on inputs neither was tuned to.

## Fix and re-verification

Not fixed in this change. The random corpus generator documents the
exclusions (FL mul/div/percent, sqrt of a non-DP-scaled radicand, zero
operands/results, memory entries with more fraction digits than DP) and the
constrained corpus passes 1000/1000 on both the firmware host core and the
original ROM through the emulator. Fixing the core means porting the ROM's
exact FL digit-emission, zero-scaling, sqrt-recall and M±-alignment rules
from the annotated listing (`$AI_HW_ENGINEER/system/busicom_141pf/spec/
reference/Busicom-141PF-Calculator_asm_rel-1-0-1.txt`, §4.4/§4.5), then
widening the corpus again as the regression gate.

## Prevention

- The corpus is permanent: `gen_random_tests.py --check` pins the programs
  to the seed, and both replay tests fail on divergence. When a region is
  fixed, remove its exclusion from the generator so the region rejoins the
  corpus.
- Rule: new arithmetic behaviour in the core gets random differential
  coverage against the original ROM, not only example-shaped tests.
