# A `zeroext i1` return came back in P2:P3 while the caller read P1

- Area: llvm-i4004 (`I4004ISelLowering.h`: return-value extension)
- Found by: the first 42-example run of the flat firmware image on
  external/chips (`run_image.sh secondary` + `compare.sh`). Example 1-1 printed
  the overflow row on its first `+`. The lit suite, `exec_cmp.py`,
  `exec_nibble.py`, `exec_num.py` and the host tests were all green.

## What broke

`operand_shown(fresh: bool) -> bool` in `src/lib.rs` returns a `bool`. In IR
that is `define ... zeroext i1 @operand_shown(...)`. The two sides lowered the
return value differently:

- **Caller:** `RetCC_I4004` promotes `i1` to `i8` and reads it in P1.
- **Callee:** the `zeroext` return goes through
  `TargetLowering::getTypeForExtReturn`. The default widens to the register
  type of `i32`, which is `i16` here. `RetCC_I4004` puts an `i16` in D1 = P2:P3.

So the callee emitted `ZEXT8TO16 → $d1` (`ld r4; xch r4; ld r5; xch r5;
fim p3, 0`), and the caller tested whatever was left in P1. `add_sub` treated
the operand as not shown and set the overflow lamp. The MachineOutliner then
put the bad sequence into a shared `OUTLINED_FUNCTION_14`, which made it look
like an outliner bug at first. Compiling with `-enable-machine-outliner=never`
showed the same bad copy.

## Root cause and why our tests missed it

The backend never overrode `getTypeForExtReturn`. No test returned a
`zeroext`/`signext` value narrower than `i8` across a real call. The lit tests
return `i8`/`i16`. The execution tests (`exec_cmp`, `exec_nibble`) call
functions that return `i8`. The Rust firmware gained its first `bool`-returning
non-inlined function (`operand_shown`) during the size work of issue #18.

## Fix and re-verification

- `I4004TargetLowering::getTypeForExtReturn` widens any type narrower than
  `i8` to `i8`, the type the caller's `RetCC` expects.
- `tests/codegen/ret_const.ll`: `retbool` checks that a `zeroext i1` return is
  `fim p1, 0x1; bbl 0` with no `fim p3`.
- The flat image (4,079 B) then ran 42/42 on chips.

## Prevention

- Rule: the callee and caller lowerings of every return type must be checked
  against each other, including the extension hooks (`getTypeForExtReturn`,
  `CCPromoteToType`). An ABI test that only checks the callee's assembly does
  not catch this.
- Add `i1`/`zeroext` return cases to the execution tests whenever the ABI
  changes. The 42-example whole-program run on chips is what caught this one.
