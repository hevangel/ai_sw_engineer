# 2026-10-19 (cont.) — far-branch relaxation DONE (chips-tested); firmware now linkable

Branch `system/busicom_141pf-on-4004`. No PR. Every tool call timeout-wrapped;
host tests 42/42 exact; lit 26/26 throughout.

## Item 1 COMPLETE — far-branch relaxation (the hard gate)
The 4004's JCN/ISZ 8-bit address field only reaches within the current 256-byte
ROM page, so functions > 256 B could not be encoded and nothing linked. New
pass `I4004BranchRelax` (after ExpandPseudos, before the peephole) rewrites every
conditional branch page-independently:
  JCN cc,Dest -> JCN !cc,.Lcont ; JUN Dest ; .Lcont:
  ISZ Rn,Dest -> ISZ Rn,.Ltaken ; JUN .Lcont ; .Ltaken: JUN Dest
build_image.sh dropped --pad-pages. Wired in I4004.h / TargetMachine /
CMakeLists. **Execution-tested on external/chips** with a 647-byte, 3-page
function whose loop back-edge + compares straddle pages
(`llvm-i4004/tests/exec/run_relax_test.sh`: result correct). Cost +1,378 B
(40,736 -> 42,114).

RESULT: the xl link now gets PAST the page-local error and fails only on TOTAL
SIZE (`ROM image 43,552 bytes exceeds budget 4096`). The whole firmware is now
LINKABLE given enough ROM — the single biggest structural blocker is gone.

## Item 2 (Tier-3 bank-switched ROM) — scoped, not yet built
~11 x 4 KiB banks needed at 43,552 B. Design: bank latch on a spare 4001 output
port; common low page(s) hold crt0/HAL/hot-rt/trampolines; cross-bank call =
JMS trampoline that saves caller bank on P7 soft stack, sets latch, JMS target,
restores latch, BBL; packer assigns functions to banks and rewrites cross-bank
calls. KEY CONSTRAINT: the native 12-bit PC already addresses 16 x 4001 = 4096 B
with NO banking (unmodified external/chips supports it via chip_index=addr[11:8]);
banking is only needed ABOVE 4096 B, and since external/chips is read-only
(no license) the chips-side banking must live in the ADAPTER
(platform/scripts/chips_trace.rs: intercept the latch write, reload Board.roms[]
with the selected bank). RTL xl top adds the latch to ROM decode.

This is a multi-hour sub-project (packer bank assignment + trampoline ABI + two
simulators + memory tests) — the next unit of work. Note: if Tiers A/B shrink
the image to <= 4096 B, banking becomes unnecessary and the image runs directly
on the 16-ROM xl board.

## Size progression
45,714 -> 45,320 (peephole) -> 41,424 (spill FIM-once) -> 40,736 (regops) ->
42,114 (far-branch relax, +correctness). Still ~10x over; Tiers A/B continue
after a running image (banking) or toward <=4096 B.

## Validation
lit 26/26; relax exec test PASS (647B/3-page fn on chips); regops chips 10/10;
host cargo test 42/42 exact; whole firmware links (fails only on size now).
