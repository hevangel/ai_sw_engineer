# The 8008 RTL took RTc (return if true) as an unconditional fall-through

- Area: ai_hw_engineer `design/intel_8008` (the primary oracle for
  llvm-i8008 and the Micral N titles); found while bringing up llvm-i8008
- Found by: llvm-i8008's execution tests on the RTL. Signed modulo and the
  shift loops in `rt/` returned wrong results, while the same code was
  correct by hand-simulation against the MCS-8 manual's opcode table.

## What broke

The RTL decoded the conditional returns with `(code & 8'he7) == 8'h03`.
That mask keeps bit 5 out of the match, but RFc is `00 0cc 011` and RTc is
`00 1cc 011`: with bit 5 masked away, the eight RTc opcodes (`043` `053`
`063` `073`) did not match at all and ran as something else. Only RFc
worked. `rt/arith.s` and `rt/shift.s` use `RTZ`/`RTC` at loop exits.

## Root cause and why its tests missed it

The RTL's directed testbench exercised RFc but no RTc; the SCELBAL
regression it also runs happened not to take an RTc on the paths it
covers. Our backend tests were the first code to depend on RTc.

## Fix and re-verification

In ai_hw_engineer (left uncommitted for its maintainers, as agreed): the
mask is `8'hc7`, with a comment citing the manual; `tb/tb_top.sv` adds an
RFC and an RTZ step. The directed test, the 200k-cycle SCELBAL regression
and lint pass. llvm-i8008 `exec_tests.py`: all cases pass on the fixed RTL.

## Prevention

Every opcode the backend emits appears in at least one execution test that
checks its effect on the RTL (`exec_tests.py` groups, `plans/testplan.md`),
so an oracle defect shows up as a failure of hand-checkable code. A
disagreement between the RTL and the manual is reported to ai_hw_engineer.
