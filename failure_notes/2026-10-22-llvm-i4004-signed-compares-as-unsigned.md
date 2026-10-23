# Signed compares were lowered as unsigned, and no test exercised one

- Area: llvm-i4004 (`I4004ISelLowering.cpp` `mapCC`, `I4004ExpandPseudos.cpp` `emitBrCC`)
- Found by: external review (issue #18, "Fable code review", RC-8), confirmed on
  external/chips by the new `scripts/exec_cmp.py`

## What broke

`mapCC` mapped `SETLT/SETGE/SETGT/SETLE` onto the unsigned codes `ULT/UGE`
("Signed relations are approximated by unsigned for the backbone (the BCD
firmware is unsigned)"). The firmware is not unsigned: `div_gen`,
`assemble_quotient`, `place_quotient_digits`, `sqrt_at` and `scale_pow` use
signed `i16` with `< 0`, `.max(0)` and `.min(0)`. Every signed relation between
operands of different sign took the wrong edge, so on target division and
square root would compute wrong digits even on a perfect simulator — a second,
independent cause of the 0/42 target result, which the final report attributed
entirely to the chips 4002 model.

Measured with the baseline llc (`exec_cmp.py`, 5,540 cases): every signed
predicate failed every mixed-sign pair — i8 41/81 per predicate, i16 branch
form 100/196.

## Root cause — and why our tests missed it

The approximation was a deliberate backbone shortcut justified by a belief
about the firmware ("unsigned BCD") that was never checked against the IR. The
runtime execution tests (1,351) and `exec_fn.py`/`exec_i16.py` covered only the
operations the author believed the firmware used — unsigned add/sub/shift/
bitwise — and no codegen or execution test compiled an `icmp s*`. The FileCheck
tests checked the *shape* (`sub`, `jcn`) of compare lowering, which is the same
for signed and unsigned, so they could not see the wrong semantics.

## Fix and re-verification

- New condition codes `I4004CC::SLT/SGE` (`SGT/SLE` canonicalize by operand
  swap, `reverseBranchCondition` maps SLT<->SGE).
- `emitBrCC` compares `(lhs ^ 0x80..)` with `(rhs ^ 0x80..)` unsigned: at the
  top nibble it flips bit 3 with `RAL ; CMC ; RAR`, which preserves the borrow
  in CY; rhs's top nibble is flipped in place and restored after the SUB. Cost:
  +13 bytes per signed compare (+65 B over the whole firmware).
- `tests/codegen/compare_signed.ll` (FileCheck) and `scripts/exec_cmp.py`
  (all ten predicates x {value, branch} x {i8, i16} over a sign-boundary matrix,
  executed on external/chips): **5,540/5,540**, wired into `run_tests.sh`.

## Prevention

- Execution coverage is per *IR operation the frontend can emit*, not per
  operation the author expects: `exec_cmp.py` runs every `icmp` predicate.
- A lowering that approximates semantics must fail loudly
  (`report_fatal_error`) rather than silently approximate.
