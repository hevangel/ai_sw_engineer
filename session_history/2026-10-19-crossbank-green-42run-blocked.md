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
To unblock, one of (ranked by cleanliness):
  (a) ELIMINATE per-callee common stubs (user-requested). A cross-bank call is
      a FORWARD jump through the EXISTING __soft_ret dispatch, done by pushing a
      second ("forward") token for the callee and `jun __soft_ret` -- no new
      common code beyond one small push helper. Design, worked out and partly
      scaffolded (then reverted to keep green):
        - add `__push_pb` (push R0=page, R1=bank; SSP+=2, 8-bit) -- ONE common
          copy. Bytes verified:
          2f a0 e0 f1 d1 8f bf d0 8e be 2f a1 e0 f1 d1 8f bf d0 8e be c0
        - the packer rewrites each cross-bank `jun callee` (2 B) IN THE CALLER's
          banked section into (12 B, uses only P0 + ACC, so args in P1-P3 and
          the already-pushed return token are untouched):
            fim p0, %lo8(callee)   ; offset -> R0:R1   (LO8 reloc)
            jms __push_off         ; push off.lo, off.hi
            fim p0, %page(callee)  ; R0 = page          (PAGE4 reloc)
            ldm <bank>             ; ACC = callee bank  (packer fills the nibble)
            xch r1                 ; R1 = bank
            jms __push_pb          ; push page, bank
            jun __soft_ret         ; pops the forward token, sets latch=callee
                                   ;   bank, dispatches to callee page:offset
          callee runs; its `jun __soft_ret` then pops the ORIGINAL return token
          (pushed by the normal CALLSOFT prologue before this sequence),
          restoring the caller's bank. Common = runtime + __soft_ret + __srtab +
          __push_off/__push_pg/__push_pb only -- NO per-callee stubs.
      The hard part is the in-place section rewrite: inserting +10 B at each
      cross-bank `jun` shifts every later byte, reloc offset (ADDR12/PAGE8/LO8/
      PAGE4), and local MBB symbol in that section, and must iterate with
      placement to a fixed point (a same-bank call can become cross-bank as the
      section grows). Per-function budget is now < 1792 B so the growth fits.
  (b) [TRIED, WRONG] Move the `__tobank` stubs into the BANKED region (one copy
      per calling bank, in that bank's reserved tail). This LINKS the whole
      firmware (verified: 40 banks, 83,968 B, common 2048/2048) BUT IS
      INCORRECT: the stub sets the latch to the callee's bank, and the chips
      bank-reload (and real hardware) then swaps the banked chips -- including
      the page holding the stub's OWN remaining `jun callee` -- out from under
      the running stub. A stub that switches the bank MUST live in COMMON (never
      swapped). The banked exec test returned garbage; reverted.
  (c) Reclaim the ~340 B of common function-padding (pad only where a JCN/ISZ
      actually crosses a page), freeing ~34 slots for the current 12-B COMMON
      stubs. Correct but only buys a one-time ~34 slots (<65 needed), so pair
      with fewer cross-bank callees.
CONCLUSION: option (a) is the UNIQUE fully-correct unblock, because the forward
switch must run in COMMON and __soft_ret already does exactly latch-set +
cross-bank `jin` dispatch there. The firmware FIT is proven achievable (option b
linked it); the only remaining work is option (a)'s in-place call-site rewrite,
which keeps common holding just runtime + __soft_ret + __srtab + 3 push helpers.

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
