# The printer fired one drum sector early on the RTL (TEST phase differs between simulators)

- Area: system/busicom_141pf/firmware (`asm/drivers.s`: `__pr_print`)
- Found by: the first 42-example run of the flat image on the primary
  simulator (ai_hw_engineer RTL, Verilator, xl board). The same image was
  42/42 on external/chips. That disagreement between the two simulators is
  the finding.

## What broke

Many printed rows came out one drum sector early on the RTL: `1`→`0`,
`.`→`9`, `0`→`-`, `C`→`%`, `+`→`◇`. Some rows printed correctly (`91.481 *`),
so the error depended on timing.

`__pr_print` waited for the drum index (ROM2 bit 0) and then started sector 0
at the first point where `JCN T` stopped looping ("TEST high"). Both simulator
adapters drive the same TEST level each half-spin (`spin % 2 == 0`). But
external/chips takes a C4 jump when its flag is high and the RTL when the pin
is low (MCS-4 manual), so the firmware sees opposite phases:

- **chips:** the index is first seen at half-spin 1, where TEST reads "high".
  The firmware waits one half-spin and fires at half-spin 2, the first half of
  the sector-0 window.
- **RTL:** at half-spin 1 TEST already reads "low = go", so the firmware fires
  at once. That is the second half of the sector-12 window.

When the printer reached the index wait at half-spin 2 instead, both
simulators fired inside sector 0, which is why some rows were right.

A second defect showed up once the phase was fixed. The last sector's hammers
(sector 12: symbol `M` in column B, `R` in column A) were printed on the *next*
row. `__pr_print` fired sector 12 and advanced the paper in the same
half-spin. The RTL board delivers a hammer and an advance that arrive in one
poll window with the advance first, so the hammer landed after the line feed.
chips decodes edges cycle by cycle and did not show it. The original ROM
leaves time between the last hammer and the advance.

## Root cause and why our tests missed it

The driver was written and debugged against chips only, and the host board
model abstracts the drum as one sector per `test_pin()` sample (spec M3-P1).
`platform/README.md` ("JCN TEST-pin polarity", point 3) says the chips adapter
drives the inverse level to compensate. It does not: `chips_trace.rs` and
`tb_trace_main.cpp` drive the same value. The original ROM matches on both
only because it does not depend on the TEST phase. We read that README as a
guarantee that the two simulators see the same phase.

## Fix and re-verification

- `__pr_print` counts TEST edges, never levels. `.Lpr_edge` waits for the
  level to change. Sector 0 starts at the first edge after the index is seen,
  and each later sector starts two edges on. This lands in the same sector
  window on both simulators, wherever the printer joins the half-spin.
- The paper advance waits one more TEST edge after sector 12.
- +12 B of driver code, +4 B of relaxation elsewhere (image 4,079 → 4,095 B).
  chips 42/42 and RTL 42/42 on the 4,095 B image.
- `platform/README.md` point 3 corrected.

## Prevention

- Rule: driver code that waits on TEST must be run on both simulators before
  it counts as verified. A pass on one simulator says nothing about the phase.
- Polarity notes have to be checked against the adapter code, not quoted.
