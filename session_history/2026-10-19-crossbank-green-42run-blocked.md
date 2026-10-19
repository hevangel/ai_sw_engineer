# 2026-10-19 — cross-bank green; 42-run blocked on stub budget; static-frame fix scoped

Branch `system/busicom_141pf-on-4004` (HEAD ~6e2c239). Continues the soft-call
work. The cross-bank soft-call return is FIXED and green; the whole-firmware
42-example run is blocked on one quantified resource wall; the static per-call
frame fix (the latent spill-corruption bug) is designed but not yet landed.

## Green and committed this stretch
- **Cross-bank soft-call return FIXED** (3 bugs): a sub-page test bank window
  omitted the return-page `jin p0` stub; a COMMON caller -> banked callee was
  not detected as cross-bank (no latch set); `__tobank` stubs keyed by address
  alone collided across banks (a banked PC exists in every bank) -> now keyed by
  (addr, bank). `tests/exec/run_softcall_banked_exec.sh` PASSES: depth-8 chain
  across banks + i16 return through a cross-bank call, latch toggling forward
  (to callee bank) and back (restored from the token's 4th bank nibble in
  __soft_ret). lit 28/28; flat soft-call exec green; host cargo test 42/42.
- **chips_trace.rs latch wired**: splits a banked image into common + per-bank
  windows, boots bank 0, reloads banked ROM chips on a chip-3 latch write.
- **Firmware shrunk**: four more #[inline(never)] splits (dispatch_key ->
  dispatch_op_key, mul_div_key -> mul_div_chain, divide -> div_gen, mul ->
  mul_accumulate); max compiled function now 1,757 B (was 1,958). Host 42/42.
- **__tobank stub reverted 10 -> 12 B**: WRR does NOT preserve ACC, so the
  10-byte shrink (which reused ACC for the __cur_bank write) corrupted the
  cross-bank forward switch. 12 bytes (re-LDM the bank) is correct.

## Item 5/6 — 42-example run: BLOCKED on the common-region __tobank stub budget
The banked firmware link needs, in the common region (mapped in every bank):
runtime+crt0+HAL ~1,390 B + the soft-call dispatch page (256 B) + **65 distinct
cross-bank `__tobank` stubs (780 B)**. That is ~2,330 B. The 4004's 12-bit PC
forces common + bank <= 4096, and the largest function is 1,757 B, so bank must
be >= 0x700 (1,792) and common <= 0x900 (2,304). At common=0x900 the free-slot
packer finds only ~39 of the 65 stub slots (function page-align padding pushes
the real 1,390 B of functions out to ~0x6c2 = 1,730 B, wasting ~340 B). So the
link is ~26 slots (260 B) short.
To unblock, EITHER:
  (a) cut the number of cross-bank callees (~65) roughly in half via a
      graph-partition bank placement (minimise cut call edges) -- a DFS+first-fit
      clustering attempt INCREASED edges and regressed the exec test, so a real
      min-cut/affinity partitioner is needed; or
  (b) reclaim the ~340 B of common function-padding (place common hand-asm
      sections tightly, padding only where a JCN/ISZ actually crosses a page),
      which would free ~34 slots -- enough; or
  (c) split more functions below a ~1,536 B window to allow common=0xA00.
Option (b) is the cleanest and lowest-risk next step.

## Item 2 — static per-call frame allocation (spill-corruption fix): DESIGNED
The backend spills to a FIXED shared RAM region (chip0/reg3 @ 0x30) for EVERY
function, so a caller's spilled value is clobbered by any callee that spills
(verified: `sumdown` recursion returns wrong; also a latent risk on deep
non-recursive paths). The firmware call graph is ACYCLIC with MAX DEPTH 16
functions (root `main`), so a static interval allocation is exact.
Plan (requested by the user, 8051-compiler style):
  1. Backend: `eliminateFrameIndex` sets the FI operand to the within-frame
     byte offset (slot*2); ExpandPseudos SPILL/RELOAD/LEA_FI emit
     `FIM p0, __frame_<fn> + off` as an EXTERNAL-SYMBOL operand (lowered by
     MCInstLower to MCSymbolRefExpr+off -> R_I4004_IMM8) instead of the constant
     0x30.
  2. Packer: build the call graph; assign each `__frame_<fn>` a RAM base so that
     functions simultaneously live on any call path never overlap (a function's
     frame is live across all its transitive callees); siblings share. Fail
     loudly on a cycle (recursion would need the reverted SSP-relative frame).
     Verify the total fits bank-0 RAM (4x 4002) and the xl RTL.
  3. Execution-test a caller+callee that both spill on chips; re-run the 42.
This does NOT unblock the 42-run (it is RAM, not the ROM stub budget) but is the
correct fix for spill correctness and must land before trusting any 42 result.
