# 2026-10-19 (cont.) — banking: two fundamental walls mapped; soft-stack cross-bank is the real fix

Branch `system/busicom_141pf-on-4004`. No PR. host 42/42 exact; packer unit 15/15
(reverted an experimental call-graph packer that churned tests for 41->36
trampolines without resolving the squeeze; tree is green at the committed state).

## Where banking stands (all prior commits this run stand green)
- Firmware links byte-wise into a 30-bank image; page-align fixed; trampolines
  are correctly address-keyed (41 distinct, section-symbol calls covered).
- The whole firmware is bank-PLACEABLE; the trampoline MECHANISM is built.

## TWO fundamental walls now fully characterised
**Wall 1 - geometry.** Per-bank VIEW is 4096 B (16-chip 12-bit PC). It must hold
common (runtime ~1,550 B + N trampolines) PLUS one bank's worth of firmware,
and the largest firmware fn is ~2,340 B. Even with a SHARED bank-switch core
(each stub ~8 B: ldm bank; jms __be; jms target; jms __bx; bbl) the common is
~1,874 B, leaving < 2,222 B for a bank -> the 2,340 B fn still does not fit.
Needs ~2 more Tier-B splits to push every fn <= ~2,100 B.
**Wall 2 - hardware stack depth (the hard one).** A cross-bank call via a
JMS trampoline adds a hardware-return-stack LEVEL (caller->stub->target). The
4004 stack is only 3 LEVELS and the firmware is already <= 3 deep by design, so
a cross-bank call at depth >= 2 (which dispatch->add_sub->... hits) OVERFLOWS
the stack. JMS-based banking cannot work for this firmware.

## The real fix (definitive): software-return-stack cross-bank calls
Route every cross-bank call through the P7 SOFTWARE return stack (abi.md 4.4),
NOT the hardware stack: the trampoline pushes the return (3 nibbles) on the soft
stack, sets the latch, and JUN's (not JMS) the target; the target returns by
popping the soft stack and JUN'ing back (a different return convention for
cross-bank-reached functions). This bypasses the 3-level hardware stack
entirely. It is a firmware/ABI-wide mechanism (the backend must emit the
soft-stack return for functions reachable cross-bank, or the trampoline must
wrap them), and is the genuinely correct path - the one the original 141-PF
never needed because it never banked.

## Honest status
Banking is NOT a few edits from running: it needs (1) ~2 more fn splits
(geometry) and (2) a soft-stack cross-bank calling convention (depth). (2) is
the substantial piece. The alternative that sidesteps BOTH walls is to keep
driving TIER-A/B size reduction toward <= 4096 B total so NO banking is needed
(the native 16-chip view runs it directly) - currently ~44 KB, far off, but it
is the only path with no new hardware-deviation mechanism.

## Validation
host 42/42 exact; packer unit 15/15; lit 26/26; regops chips 10/10. Firmware
links byte-wise (not yet executably-correct across banks, per Wall 2).
