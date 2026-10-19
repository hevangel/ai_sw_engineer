# 2026-10-19 (cont.) — page-align DONE (firmware links); CORRECTNESS GAP: 183 cross-bank calls untrampolined

Branch `system/busicom_141pf-on-4004`. No PR. host 42/42 exact; timeout on all calls.

## Done this run (committed + pushed)
- **Relaxed-JCN page-straddle FIXED.** Linker._page_align_section NOP-pads any
  PAGE8/FIN8 2-byte instruction (and its same-section target) off a 256-byte
  page boundary; _place_rom_banked runs it on banked sections to a fixed point.
  Unit test 15/15 (adds a JCN-at-0xFD straddle case).
- **The whole firmware now LINKS into a banked image** byte-wise:
  `i4004_pack.py --banks 30 --common-bytes 0x680 --bank-bytes 0x980` -> 74,624 B
  (common 0x680 + 30 x 0x980); firmware spread across banks 0-18 (common holds
  the 59 runtime/HAL/reset sections).

## CRITICAL CORRECTNESS GAP (must fix before the image can RUN)
The `--emit-trampolines` run reported "0 cross-bank trampolines", but a
from-scratch `_cross_bank_calls()` finds **183 cross-bank ADDR12 calls** (e.g.
memory_add in bank X -> rescale in bank 14, is_zero in bank 17). They were NOT
trampolined because they use SECTION-symbol relocations (`.text._R...` + addend),
and `_callee_name()` returns the section name while `cross_bank_callees()` keys
on a function name -> the 183 calls are silently resolved to the callee's banked
address with NO latch switch. So the linked image is WRONG: a bank-0 caller
JMS'ing a bank-14 address fetches whatever is mapped in bank 0 at that address.

FIX (next, precise):
1. `_callee_name` / `cross_bank_callees`: key cross-bank callees by (section,
   addend) when the reloc is a section symbol, not by function name. Generate
   `__bankcall_<sanitised-section>_<addend>` (or one per distinct target addr).
2. `emit_trampolines_source`: the trampoline's `jms` must target that
   section+addend (emit `.set`/a local symbol at the right offset, or have the
   packer stamp the resolved banked address directly since the packer knows it).
   Simplest: since the packer resolves addresses, have the trampoline JMS use a
   packer-stamped absolute addr rather than a symbol.
3. `--rewrite-cross-bank`: match the same (section, addend) key to redirect.
4. Re-link; now trampoline count ~= number of distinct cross-bank targets.
Then the chips bank test + run_image latch wiring can actually exercise banks.

## Validation
host 42/42 exact; packer-bank unit 15/15; lit 26/26. Banked image links
byte-wise (74,624 B) but is NOT yet executably-correct (gap above).

## UPDATE (same run): trampoline gap FIXED; geometry squeeze is the new blocker
- Address-keyed trampolines landed: 183 cross-bank call SITES -> 41 DISTINCT
  trampolines (1,435 B). The section-symbol correctness bug is fixed; the image
  no longer silently mis-calls across banks. Unit 15/15.
- NEW blocker (geometry): common must hold runtime (~1,550 B) + 41 trampolines
  (1,435 B) ~= 3 KB, but common+bank <= 4096 and bank >= 2,340 (divide) ->
  common <= 1,756 B. 3 KB common does not fit a 4 KB per-bank view.
- FIX (next): shared bank-switch CORE. One __bank_call core (push bank, set
  latch, indirect-call the target via JIN/FIN + soft-stack return, restore);
  each per-target stub is ~6 B (load {bank,target}, JMS core) instead of 35 B.
  41 x 6 + ~40 ~= 290 B common -> fits. Then link, chips bank test, run_image
  latch wiring, 42-example run.
