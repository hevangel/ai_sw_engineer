# 2026-10-19 — DCL-banked frame RAM (banks 1-3): backend+packer landed on wip, one deep banked case open

Worktree `B:\ai_sw_engineer_run`. Work isolated on local branch **`wip/dcl-frames`**
(cut from `system/busicom_141pf-on-4004` @ ef6700c), per the brief's
never-throw-work-away rule. NOT merged to the main work branch (the exec suite
is not fully green yet — see the open item). No PR opened.

Model-call evidence: this was a KiroCrew subagent run (model = Auto/server-
selected). Kiro does not expose per-model token totals; recording the available
fact only: one continuous subagent session, many `docker run` tool invocations
against `ai-sw-engineer:latest`. Duration: a long single session.

## Goal
Move per-function spill/reload frames off the full CM-RAM bank 0 into banks 1-3
(DCL frame scheme) so the firmware's ~420-char deepest frame path fits, then
link + run the 42 examples. (Steps 3-4 of the brief NOT reached — blocked on the
banked frame exec gate.)

## Landed on wip/dcl-frames (commits 9c1e7fa, f1a12e7, e8a707d)
Backend `I4004ExpandPseudos.cpp` (DCL frame scheme):
- `LEA_FI` now puts the frame's DCL bank code in **ptr.hi.lo** via a new
  absolute linker symbol `__framebank_<fn>` (R_I4004_IMM8; value 0/1/2/4 for
  banks 0/1/2/3). Low pair stays `__frame_<fn>+off` (within-bank char index).
- `LDRAMi8/STRAMi8/LDRAMi16/STRAMi16` read ptr.hi.lo and bracket the access
  with `LD bank; DCL … LDM 0; DCL`. A bank-0 pointer carries code 0, so the
  DCL is a harmless re-select of bank 0 — uniform, no special-casing.
- `SPILL8/16` + `RELOAD8/16` bracket with `FIM P0,__framebank_<fn>; LD r1; DCL
  … LDM 0; DCL`. DCL is always restored to 0 before control can reach a
  bank-0-assuming helper (soft stack, __cur_bank, the CALLSOFT push between two
  spills).
New helpers `emitFrameDcl` / `emitFrameDclReset` / `emitDclFromReg`.

Packer `i4004_pack.py`:
- `_alloc_frames` remaps the call-graph OVERLAY's linear base onto usable
  CM-RAM bank windows (default banks 1,2,3 = 768 chars; bank 0's free 0x20..0x80
  also selectable) with a monotone boundary-snapping shift that never reorders,
  so simultaneously-live frames stay disjoint. Defines `__framebank_<fn>` = DCL
  code. NO new reloc machinery — reuses R_I4004_IMM8 + linker_syms, same pattern
  as the existing `__frame_<fn>`.
- New `--frame-banks` CLI (default `1,2,3`; pass `0` for flat bank-0).
- FIXED a remap double-counting bug (added both the inter-frame gap AND the
  frame size, spacing frames 2x too wide — would overflow 768 chars on the real
  firmware). Now matches the committed tight overlay.

Test harness `run_frame_exec.sh`: builds **16 RAM chips** (4 CM-RAM banks ×4),
not the old 2 — `Board::new` panics for >4 chips (u2 page_mask), so the chips
are built by hand `RAM::new(u2::new(i%4))`, exactly like `run_ramdcl_exec.sh`.
Added flat-frame / banked-frame / banked-frame+farcall variants.

## Verified GREEN (execution on the chips model, in-container)
- All pre-existing exec tests unchanged: `run_softcall_exec`,
  `run_softcall_banked_exec`, `run_widebank_exec`, `run_ramdcl_exec` PASS;
  `test_farcall.py` PASS; `test_pack_banks` 12/15 (the documented baseline).
- Controlled DCL-frame cases I wrote, PASS both flat (bank 0) AND banked
  (frames in banks 1-3):
  - depth-1 spill (`top→inc1`): result 11 ✓.
  - depth-5 i8 chain, 1-nibble result (`f1(1)=5`) ✓.
  - depth-5 i8 chain, 2-nibble result (`f1(1)=45=0x2d`) — proves the spilled
    i8 HIGH nibble survives the bank switch ✓.
  - depth-2 reload-after-call, banked (`top(5)=18`) ✓.
So the DCL spill/reload/deref logic, the high-nibble integrity, and multi-bank
DCL selection are all correct in isolation.

## OPEN — deep computed-arg chain fails ONLY with banked frames
`run_frame_exec.sh` g1..g5 (and a depth-5 chain that passes a COMPUTED arg
`a=x+1` kept live across the call) fails when frames are in banks 1-3; the
IDENTICAL program with `--frame-banks 0` (bank-0 frames) PASSES. Depth ≤2 passes
banked; depth 5 fails banked.

Precise localisation (via PC trace diff of the SAME llc output linked
`--frame-banks 0` vs `1,2,3` — the two images differ ONLY in the frame FIM
immediate bytes, bank 00→01 and offset 0x2X→0x0X):
- The soft-call RETURN token for `entry→f1` gets its **off.lo corrupted from
  0x25 to 0x05** (the 0x20 bit cleared) in the banked build. `__soft_ret` then
  dispatches back to entry's START (0x805) instead of its `.LBB5_1` return site
  (0x825), so `entry` re-runs and re-calls the chain — f1 is entered 8× — i.e.
  an unbounded re-entry, not a wrong arithmetic result.
- entry has NO frame (framesize 0, no DCL bracket), so entry's push of the
  token is provably correct; the corruption happens on the bank-0 soft stack
  BETWEEN push and pop, implying a frame/stack access that ran under DCL≠0
  somewhere in the f1..f5 chain. The generated f1..f5 assembly, however, resets
  DCL to 0 before every CALLSOFT push and before every `jun __soft_ret`, and
  the spill/reload brackets are byte-correct — so the slipping access has not
  been identified. Suspects not yet ruled out: (a) the per-page `jin p0` stub /
  `__srtab` dispatch or a trampoline touching bank-0 RAM at depth; (b) a
  frame access the ExpandPseudos pass does not cover (some other spill form);
  (c) an interaction with the 5-nibble banked return token's SSP math at depth.

Attempted fix that did NOT work and was reverted: adding `LDM 0; DCL` guards to
the start of the synthesised `__soft_ret` / `__push_off/pg/pb/1` / `__save_p4p5`
helpers. It broke the FIXED dispatch-page layout (`__srtab` pinned at 0x80 and
hardcoded helper offsets in `_emit_softcall`), regressing even the flat case.
A correct version must keep those offsets fixed (pad to absorb the +2 bytes) or
force DCL=0 at the compiled-function level instead.

## Reproduce / continue (all scripts under $KIROCREW_SCRATCH/build in-container)
- `build/passarg.sh 0` (PASS) vs `build/passarg.sh 1,2,3` (FAIL) — minimal repro.
- `build/tracelog.sh <img>` → PC trace; `build/tdiff.sh` → first divergence.
- `build/build_llc.sh` force-rebuilds llc (NOTE: the overlay copytree does not
  advance mtimes reliably, so ninja SKIPS recompiling I4004*.o — the script
  force-deletes `*Target/I4004*/*.o` first. This cost real time to discover;
  always force-delete the target objects after an overlay edit.)
- LLVM build volume `ai-sw-llvm-build` is configured; incremental llc build ~1m.

## Next step (concrete)
RESOLVED the root cause (next session: 2026-10-19, continued). The off.lo
corruption is NOT a DCL-slip in our generated code — it is a simulator
deviation: `external/chips`' 4002 (`ram4002.rs::set_register_control`, called on
EVERY chip by `mcs4/mod.rs::run_cycle` on an SRC) latches the SRC select code
WITHOUT checking the CM-RAM (DCL) bank line, so a bank-1 frame SRC clobbers the
address latch of the same-chip-select bank-0 stack chip. The ai_hw_engineer RTL
4002 (`design/intel_4002/src/intel_4002.sv`) guards the latch AND the response
with `cm_ram_i` and is correct. Written up in
`failure_notes/2026-10-19-chips-4002-src-latch-ignores-dcl.md`.

Remaining work to close the brief (exec gate still RED, so NOT merged):
1. Give the exec suite + the 42-run a FAITHFUL 4-bank RAM model. A naive
   "gate the SRC latch by DCL" patch (proven in build/patch_chips.py) makes the
   deep chain return but REGRESSES `big-chain` — the chips model decides
   `active` at SRC time and never re-checks cm_ram at I/O time, so faithful
   emulation must evaluate `selected = cm_ram && addr_match` at the WRM/RDM
   instant. `external/chips` is read-only; do it in OUR
   `platform/scripts/chips_trace.rs` (it already owns the ROM-bank swap — add a
   correct 16×4002 bank-gated RAM path, replacing `Board`'s RAM), and mirror it
   in the exec-test harnesses. OR bring up the RTL 4002 as the banked-frame
   oracle. `chips_trace.rs` also can't build >4 rams via `Board::new` today
   (u2 page panics) — same hand-built-rams workaround as run_ramdcl_exec.
2. Then: whole exec suite green with frames in banks 1-3, merge
   `wip/dcl-frames` into `system/busicom_141pf-on-4004`, push both, relink the
   firmware (report ROM banks/bytes + frame RAM per bank), run the 42 examples
   (`run_image.sh secondary --board xl` + `compare.sh`), report N/42 + first
   divergence, debug toward 42/42.

Build hygiene DONE this session: `build_llvm.sh` now force-deletes the I4004
target objects after the overlay so ninja cannot skip recompiling edited backend
files (committed).
