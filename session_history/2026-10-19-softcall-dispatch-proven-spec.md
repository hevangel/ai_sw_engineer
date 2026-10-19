# 2026-10-19 (cont.) — software-call convention: BOTH return-dispatch proofs green; spec + __push_ret landed

Branch `system/busicom_141pf-on-4004`. No PR. host 42/42 exact. HEAD f73606f.
Every llc/mc/chips/cargo call timeout-wrapped.

## Committed + pushed this run (in order)
- 34b8ce6  TOP finding (hardware-stack overflow, 395 JMS, depth 6+) + depth-4
           single-page soft-call core proven on chips (RAM[0]=7).
- 06585cb  MULTI-PAGE soft-return dispatch proven on chips (softcall_mp.s):
           page-indexed `__srtab` JUN table + per-page `jin p0` stub at fixed
           offset 0xF0; a page-0 fn soft-calls a page-1 leaf, returns across the
           page boundary correctly (RAM[0]=5). Hardest piece now proven both
           single- and multi-page.
- 10f50bc  abi.md 4.4.1 (concrete PROVEN sequences: 3-nibble token; CALLSOFT =
           fim off+page, jms __push_ret, jun callee; return = jun __soft_ret ->
           pop -> __srtab[page] -> per-page jin-p0 stub) + 4.4.2 packer/backend
           contract.
- f73606f  rt/softcall.s: __push_ret (layout-independent push helper, assembles
           to 13 B). __soft_ret/__srtab/per-page stubs are PACKER-synthesized
           (only the linker knows page layout).

## Remaining integration (large, multi-cycle; fully specified against the proofs)
1. **Packer synthesis** (i4004_pack.py, like trampoline synth): reserve a
   1-byte `jin p0` stub at a fixed offset (__SRT_STUB_OFF) on EVERY ROM page
   (common + every bank); emit the stubs; synthesize __soft_ret's dispatch tail
   + the 16-entry __srtab (page-local to __soft_ret) with concrete offsets
   (do NOT hand-write __srtab offset math in asm — it was brittle; let the
   packer emit it with known addresses). With banking, __soft_ret also writes
   the bank latch from the token's bank nibble before dispatch.
2. **Backend**: lower i4004_call -> CALLSOFT pseudo (fim ret-off + page; jms
   __push_ret; jun callee; .Lret) expanded post-RA; every NON-LEAF function
   return -> `jun __soft_ret` instead of BBL. Keep JMS/BBL ONLY for leaf
   libi4004rt/HAL helpers (depth<=3). Needs LLVM rebuild
   (JOBS=16 build_llvm.sh, minutes).
3. **Packer assert**: JMS-reachable subgraph is <=3 deep (leaf rule).
4. chips tests: depth-8 plain + cross-bank chain; recursion depth 5; return
   values in P1/D1 preserved.
5. Banking folds in (bank nibble in token + latch in __soft_ret); the 41
   JMS-trampolines disappear -> banking Wall 2 dissolved, common region shrinks.
6. Relink firmware --banks; 2-3 bank chips test; wire latch into
   platform/scripts/chips_trace.rs; run 42 examples + compare.sh; first divergence.

## Validation now
softcall_core chips depth-4 PASS (RAM[0]=7); softcall_mp cross-page PASS
(RAM[0]=5); __push_ret assembles. host 42/42 exact; packer unit 15/15; lit
26/26; regops chips 10/10.

## Unavailable this session
MCP @kirocrew-computer (declared, not configured) — reported, continued.
