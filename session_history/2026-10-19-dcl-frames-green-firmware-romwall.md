# 2026-10-19 — DCL-banked frames GREEN on both oracles; deep-corruption root cause fixed; firmware link hits a banked-frame ROM-growth wall

Branch `wip/dcl-frames` → merged (fast-forward) into
`system/busicom_141pf-on-4004` @ **e0d7a74**, both pushed to origin. Continues
`2026-10-19-dcl-banked-frames-wip.md`.

Model-call evidence: one KiroCrew subagent session (model = Auto/server-
selected). Kiro does not expose per-model token totals; recorded fact only:
one continuous session, many `docker run` tool invocations against
`ai-sw-engineer:latest`; the warm `ai-sw-llvm-build` volume gave ~1-2 min
incremental llc rebuilds.

## Root cause of the deep-call corruption — NOT a DCL/SRC issue

The prior session concluded the deep banked-frame corruption was the chips 4002
SRC-latch deviation and planned a faithful RAM model. The brief reframed it as a
DCL/SRC discipline + peephole bug. BOTH were wrong about the mechanism. A PC
trace of a minimal depth-3 chain (built fresh so image and map agree) showed the
real bug:

- The packer (`_place_rom_banked`) bin-packs function bodies contiguously into
  each bank with NO awareness of **STUB_OFF (0xFC)**, the per-page `jin p0`
  return-dispatch stub that `_synth_bank_stubs()` writes UNCONDITIONALLY at
  `p*PAGE + 0xFC` on EVERY page.
- `entry` was placed at 0x8e6..0x906, spanning 0x8FC. The stub write (a) corrupted
  entry's byte there and (b) made entry FALL THROUGH into `jin p0` (opcode 0x31),
  which jumps to P0 — a self-loop. The soft-call return dispatch walked
  g3→g2→g1→entry correctly, then at entry's resume site looped `jin p0 → 0x8F2`
  forever (functions re-entered unboundedly), surfacing as a wrong/zeroed result.
- It was never banking-specific: the deep FLAT-frame case failed too. Depth 2
  passed; depth 3+ (larger functions that straddle a 0xFC) failed. This is why
  `softcall_exec` (smaller functions) passed while the frame tests failed.

### Fix (commit 597b515, refined e0d7a74)
`i4004_pack.py`: new `_stub_pad()` reserves the 1-byte stub during PLACEMENT
(banked bin-pack + common code loop), bumping a SINGLE-PAGE section that
straddles a page's 0xFC to the next page. No-op for a multi-page section
(handled by the existing page-relaxation; a page a large function fully owns is
not a dispatch landing page) — so a case that already linked and ran
(`run_widebank_exec`'s 525-byte callee) is never regressed. `entry` moved
0x8e6→0x900, loop gone.

This ALSO resolved **open finding 1** from `...-farcalls-landed-42run-ramwall.md`
(the far-call deep i8 chain N-2 regression in `run_softcall_banked_exec`): same
stub-fallthrough class in the far-call trampolines. `run_softcall_banked_exec`
(BANKW=0x100, every call cross-bank, depth 8) now returns chain8=8.

## Peephole DCL/SRC invariant (commit 597b515) — correct, not this bug
`I4004Peephole.cpp` now invalidates the tracked SRC address latch on `DCL`
(CM-RAM bank change) and on `JMS/JUN/JIN/BBL/RETpseudo/INLINEASM`. The 4002 SRC
latch is per-chip and CM-RAM(DCL)-gated, so "SRC points at pair pN" is not
bank-independent across a DCL; a bank-0 SRC after a banked frame's DCL bracket
must never be elided. This was correct-by-construction (the emitted brackets
re-FIM P0 so no SRC was actually being elided in practice) but is the right
safety invariant and is pinned by a lit test.

## Tests (brief steps 1-2)
- `tests/codegen/dcl_src_discipline.ll`: FileCheck test pinning that every
  DCL-bracketed RAM access re-SRCs. **codegen lit 12/12, mc lit 17/17.**
- `tests/exec/src_dcl_assert.rs`: reusable SRC-after-DCL violation detector for
  the chips exec harnesses (brief's "assertion mode"). Read side (RDM/RD0-3/SBM/
  ADM) via chips' `log` trace; WRITE side (WRM/WR0-3/WRR/WMP — which chips does
  NOT trace) via per-cycle RAM diff; reads the Acc printed before each DCL so a
  same-bank re-select is not a "change". PROVEN to fire on a deliberate
  `ldm1; dcl; wrm`-without-SRC image (1 violation, exit 3) and 0 on conforming
  code. Wired into `run_frame_exec.sh` (env `I4004_ASSERT_SRC_DCL=1`).
- **Whole exec suite GREEN with the assertion ON:** `run_frame_exec`
  (flat-frame, banked-frame banks 1-3, banked-frame+farcall — all PASS, 0
  violations), `run_softcall_exec`, `run_softcall_banked_exec`,
  `run_widebank_exec`, `run_ramdcl_exec` PASS; `test_farcall` PASS;
  `test_pack_banks` 12/15 (unchanged documented baseline). `exec_fn.py`/
  `exec_i16.py`/crosscheck are PRE-EXISTING failures (6/20, 5/20) independent of
  this work — verified identical with the baseline peephole; the crosscheck
  self-test already declares the chips harness "not trustworthy" for those.

## Firmware link (brief step 2) — mitigated one wall, hit the architectural one

Linking the whole firmware (`BOARD=xl BANKS=72 COMMON_BYTES=0x800
BANK_BYTES=0x800`, frames banks 1-3) first failed on a ROM-bank wall, then on a
RAM-frame wall:

### Wall 1 (ROM, FIXED): banked-frame bracketing bloated a function past a bank
The DCL-banked scheme brackets EVERY spill/reload with `FIM __framebank; LD;
DCL ... LDM 0; DCL`. `bcd::isqrt_digits` had 69 brackets / 138 DCLs = 0x869
(2153 bytes) > the 2048-byte bank. Fixed by two new peephole passes
(commit 1b35e4c): pass 2a removes a dead reset DCL (overwritten by a later DCL
before any RAM I/O / barrier); pass 2b tracks the symbolic bank (Int / Sym
`__framebank_<fn>`) and removes a DCL re-selecting the already-active bank plus
its dead `FIM __framebank; LD` setup. isqrt_digits 2153 -> 2016 (< 2048).
Correctness unchanged: lit 29/29, whole exec suite green with the assertion ON.

### Wall 2 (RAM, ARCHITECTURAL — the real blocker): a single frame > one bank
    i4004_pack: link error: frame overlay: 69 per-function frames (total 420
    chars, deepest path) do not fit the usable CM-RAM frame windows
    [(1,0x0,0x100),(2,0x0,0x100),(3,0x0,0x100)]; largest single frame is
    bcd::Num::divide (356 chars). Add a frame bank or shrink spilling.

The frame sizes (from `__framesize_<fn>`):
  - bcd::Num::divide            = **356 chars**
  - bcd::build_div_operands     = **320 chars**
  - bcd::isqrt_digits           = **290 chars**
  - run::<BoardHal>            = 208
  - bcd::Num::mul               = 196 ... (rest < 256)

A CM-RAM bank is 256 chars and an SRC address is bank-local, so a single
function's frame CANNOT exceed 256 chars no matter how many frame banks exist —
adding a bank does not help divide/build_div_operands/isqrt_digits. The i4004
has ~6 usable index registers, so these BCD routines spill enormously. This is
finding 2's architectural wall, now exactly quantified: THREE functions have a
single-frame > 256.

The remaining work to reach the 42-run is firmware-/frontend-side, NOT the DCL
toolchain this brief covered:
  1. **Shrink the three oversized frames below 256 chars** — split
     `Num::divide`, `build_div_operands`, `isqrt_digits` with
     `#[inline(never)]` helper extraction (each helper gets its own <256 frame),
     or lighten regalloc so they spill less. This is the most tractable and is
     pure firmware Rust + rebuild-IR, no backend change.
  2. OR a cross-bank frame scheme (a frame spanning >1 bank, with the DCL bank
     recomputed per slot from the slot's absolute char index) — substantial
     backend work in ExpandPseudos' LEA_FI/SPILL/RELOAD lowering.

Option 1 is the right next step; it is firmware engineering (changing the BCD
source), deliberately out of scope for this DCL-discipline brief, so it is left
for a firmware-focused session rather than guessed at here.

## Reproduce / continue
- Exec suite: `export KIROCREW_SCRATCH=/workspace/build; I4004_ASSERT_SRC_DCL=1
  bash llvm-i4004/tests/exec/run_frame_exec.sh` (and the others) in the
  ai-sw-engineer container with `-v ai-sw-llvm-build:/build`.
- Firmware link: `BOARD=xl BANKS=72 COMMON_BYTES=0x800 BANK_BYTES=0x800
  I4004_FRAME_DEBUG=1 bash llvm-i4004/scripts/build_image.sh <fw> <out> link_xl.ld`.
- Depth-N minimal repro + PC trace: `build/trace/d3_clean.sh` (gitignored).
- The 42-run (`run_image.sh secondary --board xl` + `compare.sh`) is not reachable
  until the firmware links; it was ALSO blocked before this session (finding 2),
  so this is not a regression.
