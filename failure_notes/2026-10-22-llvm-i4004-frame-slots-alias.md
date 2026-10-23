# Every frame slot of a function aliased at `__frame_<fn>+0`

- Area: llvm-i4004 (`I4004ExpandPseudos.cpp`: SPILL8/16, RELOAD8/16, LEA_FI)
- Found by: the new compare execution test (`scripts/exec_cmp.py`) on
  external/chips while fixing the signed compares of issue #18; the review's
  "shared spill slot" item (RC-8) pointed at the same symptom

## What broke

`eliminateFrameIndex` computes a correct, distinct char offset per frame object
(verified with `I4004_FRAME_DEBUG`: 0, 4, 8, ...), but the expander built the
FIM operand with

```cpp
BuildMI(...FIM).addReg(P0).addExternalSymbol(FrameSym.c_str(), Off);
```

`MachineInstrBuilder::addExternalSymbol`'s second parameter is the operand's
**target flags**, not an offset. The offset was stored as an unknown flag and
the symbol was emitted as plain `__frame_<fn>` (MIR shows
`target-flags(<unknown>) &__frame_f`). Every spill, reload and frame address in
a function therefore hit char 0 of its frame, so any function holding two live
spilled values returned whichever was stored last. The packer's frame sizing
(`max(addend)+2`) saw only addend 0 as well.

On chips: every i16 `zext (icmp ..)` (which spills both arguments around the
select diamond) failed — `eq16` 14/196, `ult16` 105/196 — with the baseline llc.

## Root cause — and why our tests missed it

The FileCheck tests matched `fim p0, __frame_` without an offset, and the i8/i16
execution tests used straight-line functions small enough to need no spills.
No test executed compiled code with two simultaneously spilled values. The
symptom had been seen before (the 2026-10-19 SSP-relative spill fix that was
"proven and reverted", recorded as a latent shared-slot risk) but was
attributed to the frame *scheme* rather than to this API misuse.

## Fix and re-verification

- `frameSymOperand(Off)`: `MachineOperand::CreateES` + `setOffset(Off)`; used by
  all three sites. Asm now shows `fim p0, __frame_f+4`, `+8`, ...
- `tests/codegen/frame_offsets.ll` checks distinct offsets;
  `scripts/exec_cmp.py` i16 value forms now 196/196 on chips.

## Prevention

- Execution tests must include functions with register pressure high enough to
  spill (the i16 select forms in `exec_cmp.py` do).
- When a symptom is "latent", reproduce it on the simulator before choosing a
  design workaround.
