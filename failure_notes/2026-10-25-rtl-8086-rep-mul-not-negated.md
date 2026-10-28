# RTL 8086 does not negate the product of a REP-prefixed MUL/IMUL

- Date: 2026-10-25
- Area: platform / simulator (`../ai_hw_engineer/design/intel_8086`), found while bringing up `system/ibm_pc/emulator`
- Found by: a second implementation. The differential fuzz of the Rust 8088 core
  against the RTL (`system/ibm_pc/emulator/scripts/rtl_diff.sh`), with MartyPC as the
  tie-break reference.

## What broke

433,500 fuzz states agreed between the Rust core and the RTL except 218, all of
the form `F2|F3 F6|F7 /4|/5` (REP/REPNE before `MUL` or `IMUL`, byte or word): the
products differ by their sign. The Rust core negates the product, the RTL does not.

## Root cause — and why our tests missed it

On the real 8086 a REP/REPNE prefix sets an internal flag that the multiply and
signed-divide microcode uses to negate the result. The RTL implements this for
`IDIV` (the physical-chip corpus has 261 REP-prefixed `IDIV` vectors and the RTL
passes them) but not for `MUL`/`IMUL`. The corpus has no REP-prefixed `MUL`/`IMUL`
vector, so the RTL's silicon test could not see it. MartyPC's microcode-level core
(`external/martypc`, `crates/lib/marty_core/src/cpu_808x/execute_fn.rs`, "Negate
product if a REP prefix is present") agrees with the Rust core.

The behavior is undocumented, so it is outside the RTL's stated contract (the
manual does not describe it); the question is which side matches silicon. Neither
side is proven: there is no physical-chip vector for it. The Rust core follows
MartyPC and records the choice as assumption E-A5.

## Fix and re-verification

None applied to the RTL (it lives in another repository). The Rust core keeps the
negation and documents it. To settle it, capture REP-prefixed `MUL`/`IMUL` vectors
from a physical chip (ArduinoX86) or find a silicon trace, then correct whichever
side is wrong.

## Prevention

`scripts/rtl_diff.sh` is the standing check: it reports this class on every run. If
the RTL is changed, its REP-MUL/IMUL cases must go to zero; if the physical chip
disagrees with the Rust core, change the core, ledger E-A5 and the cpu.rs comment
together.
