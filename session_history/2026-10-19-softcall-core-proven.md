# 2026-10-19 (cont.) — TOP FINDING: hardware-stack overflow; software-call core PROVEN on chips

Branch `system/busicom_141pf-on-4004`. No PR. host 42/42 exact; every call timeout-wrapped.

## THE finding (reframes the whole project)
The backend lowers EVERY compiled call to JMS (395 in the firmware .s), but the
call graph is ~6+ deep (run->press->dispatch->add_sub->add_mag->reg_add...),
far past the 4004's 3-LEVEL hardware return stack. A linked image -- flat OR
banked -- overflows the stack at runtime. **No compiled image can run correctly
until the software-return-stack call convention (abi.md 4.4) is implemented.**
This is the #1 blocker to a RUNNING image, ahead of size/banking. (It also
dissolves banking "Wall 2": once calls use the soft stack, cross-bank calls add
no hardware-stack level.)

## PROVEN this run (committed + pushed)
`llvm-i4004/rt/tests/softcall_core.s`: hand-written software-call core -- a call
pushes a return token on the P7 soft stack and JUN's the callee; return goes
through a common `__soft_ret` that pops the token and JIN's back
(PC[7:0]<-pair, page kept). A depth-4 chain main->f3->f2->f1 (would overflow the
3-level hardware stack with JMS) returns correctly on external/chips: RAM[0]=7.
The hardest, most novel piece -- the JIN return dispatch -- works.

## Remaining to a correct running image (next, in order)
1. **Multi-page return dispatch.** The proof returned only to page 0. Generalise
   `__soft_ret`: token = 3 nibbles (offset lo, offset hi, page). Reach the
   return PAGE via a page-indexed table of `JUN <page_stub>` in __soft_ret's
   page; each page has a reserved 1-byte `JIN P0` stub at a fixed offset; the
   packer RESERVES that byte on every ROM page (common + every bank) and emits
   the stubs + the table. Mind the end-of-page quirk for the stub placement.
2. **Backend CALLSOFT.** New pseudo expanded post-RA: push the 3-nibble return
   token (the return label's addr), JUN callee; function return -> JUN
   __soft_ret instead of BBL. Keep JMS/BBL ONLY for leaf libi4004rt/HAL helpers
   that keep hardware depth <= 3; packer asserts rt helpers call only leaves.
3. chips-test: depth-8 plain + cross-bank chain, recursion depth 5, return
   values in P1/D1 preserved.
4. Banking folds in: bank nibble in the token + latch write in __soft_ret; the
   41 JMS-trampolines disappear (cross-bank is just the soft-call with a latch).
5. Relink firmware; 2-3 bank chips test; wire latch into chips_trace.rs; run the
   42 examples + compare.sh.

## Validation
softcall core chips: PASS (depth 4, RAM[0]=7). host 42/42 exact; packer unit
15/15; lit 26/26; regops chips 10/10; relax/bank-latch exec PASS.
