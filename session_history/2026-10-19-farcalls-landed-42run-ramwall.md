# 2026-10-19 — far-calls landed (option a); 42-run blocked on a RAM-capacity wall

Branch `system/busicom_141pf-on-4004`, HEAD **73b8a06** (pushed). Continues the
cross-bank soft-call work from `2026-10-19-crossbank-green-42run-blocked.md`.

## Landed and committed (73b8a06)
Far-calls everywhere (the design note's option a), entirely in the packer +
one small reloc number, no LLVM rebuild for this part:

- **R_I4004_BANK4 (reloc 9)** — the callee's placed-bank nibble, packer-
  resolved (bank numbers exist only after link placement). In I4004.def, the
  packer reloc table + `_patch`, asserted by `llvm-i4004/asm/tests/test_farcall.py`.
- **Far thunk** (in the caller's bank; common for a common caller or when the
  bank tail is full): saves P4:P5 (R8..R11) to RAM, pushes a forward token
  `(trampoline off, page, callee_bank)` via `__push_off`/`__push_pb`, then
  `jun __soft_ret`. It never touches the latch, so no running instruction is
  swapped out — the fatal flaw of option b (stubs-in-banks). `__push_pb` is one
  new common helper.
- **Restore trampoline** (one per cross-bank callee, callee bank / common):
  reloads P4:P5 then `jun callee`, so the 2nd i16/ptr arg (D2) survives
  `__soft_ret` clobbering P4:P5 during the forward dispatch. VERIFIED: the i16
  cross-bank case `wide=0x0406` in `run_softcall_banked_exec` is green.
- **Per-bank thunk_reserve** so thunks/trampolines fit in their own bank
  instead of overflowing common (softcall-only; legacy banked paths unchanged —
  `test_pack_banks` back to baseline 12/15).
- **run_image.sh** accepts banked images (>4096 B) on the secondary (chips) sim.

Result: the whole firmware **links and runs end-to-end across 72 banks on
chips** (149,504 B image, no crash across bank switches) — the quantified
common-stub-budget blocker is gone. lit 17+11, flat softcall exec (chain8=8,
wide=0x0406), test_farcall all green.

## Open finding 1 — far-call DEEP i8 chain (regression in run_softcall_banked_exec)
`run_softcall_banked_exec` (a stress test: BANKW=0x100 forces EVERY call cross-
bank at depth 8) now returns chain8 = 0 under far-calls; legacy `--no-far-calls`
still passes it. A depth sweep shows a constant **N-2** loss (depth 3→1, 5→3,
8→6). Removing the P4:P5 save/restore trampoline made chain8 pass (but then the
i16 case failed), so the defect is in the restore-trampoline path for deep i8
chains specifically. The i16 case (depth 2) is correct; the real firmware's
cross-bank calls are shallower/fewer, but this must be root-caused before the
42-run can be trusted. The far-call commit therefore knowingly regresses this
one stress test relative to ff7b641 — recorded here rather than hidden.

## Open finding 2 — static per-function frames exceed the 4004's addressable RAM
Implemented (NOT committed; reverted to keep 73b8a06 clean, LLVM rebuilt back):
- backend `eliminateFrameIndex` emits `__frame_<fn> + slot*2` as an external
  symbol (→ R_I4004_IMM8) for SPILL/RELOAD/LEA_FI instead of the fixed 0x30;
- packer `_alloc_frames()` call-graph OVERLAY: `base(fn)=max over callers
  (base+size)` in topo order, recursion fails loudly, frame size = max slot +4
  chars (a SPILL16 walks 4 chars from the one relocated base).
This is correct (flat softcall exec green; overlay bases verified disjoint) but
**the firmware does not fit**: an SRC address is 8 bits, so a spill reaches only
the current CM-RAM bank (256 chars) without a DCL switch, yet the deepest call
path's combined frames need RAM up to **0x132** (258 chars) — and the heaviest
single function's frame alone is ~255 chars (the i4004 has ~6 usable registers,
so the Rust code spills enormously). It also collides with the soft stack
(0x100). So per-function static frames as specified cannot fit this spill-heavy
build on one RAM bank.

Next steps (pick one): (a) root-cause the far-call deep-chain N-2 defect; then
(b) for frames, either shrink spilling (lighter regalloc / more
`#[inline(never)]` splits so each function's frame is small), or a DCL-banked
frame scheme (every spill/reload selects a frame bank), or validate on the RTL
(primary) sim which models all 16 RAM chips (4 banks) rather than the chips
secondary model's single bank.
