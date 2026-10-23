# Fixed-point division lost digits when the divisor had more decimals

- Area: system/busicom_141pf/firmware (`bcd::Num::div`, its asm twin `__n_div`,
  and the pre-#18 reference `tests/legacy/bcd_old.rs`)
- Found by: review of the division algorithm while compacting `__n_div` for the
  4 KB fit (issue #18), confirmed with a host probe; not by any test

## What broke

`1 ÷ 0.003` at DP 2 printed `333.00` (333.30 with the guard digit, rounded)
instead of `333.33`. In fixed mode the long division ground `frac + 1` quotient
digits of the digit strings, `A·10^(frac+1) / B`. The real quotient is that
times `10^(sb − sa)`, so when the divisor had more fractional digits than the
dividend (`sb > sa`), the last `sb − sa` places were never computed and were
zero-filled when the window was aligned. FL mode was right, because its digit
count is derived from the operands' magnitudes, scales included.

## Root cause and why our tests missed it

The M2 implementation (now `tests/legacy/bcd_old.rs`, `div_gen`) had the bug.
The Layer B rewrite reproduced it faithfully, and so did the asm twin. Every
check compared one of those against another: `tests/bcd_equivalence.rs`
(new vs legacy), `exec_num.py` (asm vs new). The only independent oracle is
the original ROM's tape for the 42 manual examples, and none of them divides,
in fixed mode, by a number with more decimals than the dividend.

## Fix and re-verification

- `gen_frac = frac + max(0, sb − sa)` in `Num::div_host`, in `__n_div` and in
  the legacy reference, which is documented as corrected.
- New regression test `div_divisor_scale_shortfall` (`tests/bcd_equivalence.rs`):
  333.33 at DP 2, and 13 significant digits in FL.
- Host `cargo test` green (42/42 manual examples unaffected); `exec_num.py`
  `__n_div` 300/300 against the fixed twin on chips.

## Prevention

A differential test is only as good as its reference. When the reference is
our own earlier code, add property checks that do not depend on it. Here: for
a fixed-mode quotient q of a ÷ b at `frac` places, q·b ≤ a < (q + ulp)·b.
That check is now on the list for `tests/bcd_equivalence.rs`.
