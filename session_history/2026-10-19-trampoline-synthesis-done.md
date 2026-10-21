# 2026-10-19 (cont.) — cross-bank trampoline synthesis DONE (13/13 unit); 2 integration items

Branch `system/busicom_141pf-on-4004`. No PR. timeout on every call; host 42/42.

## Done this run (committed + pushed)
1. **Tier-B split COMPLETE** (prior commits this run): every compiled firmware
   function <= 2,470 B (run 8276 / divide 4815 / sqrt_at 3906 / print 3497 all
   broken up via inline(never) + extracted restoring_divide/assemble_quotient/
   build_div_operands). Firmware fits common(1.5 KB)+2.5 KB-bank. 42/42 exact.
2. **Cross-bank trampoline synthesis** in i4004_pack.py:
   - `--emit-trampolines FILE` (pass 1): place into banks, write trampolines.s
     with one `__bankcall_<callee>` per cross-bank callee. Trampoline (common
     region) saves active bank (__cur_bank RAM cell) on the P7 soft stack, sets
     the ROM-chip-3 latch to the callee bank, JMS callee, restores, BBL -
     preserving P1..P6 / D1 / D2 (values ride ACC across SRC; only P0/P7/ACC
     touched). Verified the template assembles (35 B) and is ACC-coherent.
   - `--rewrite-cross-bank` (pass 2): link trampolines.o, rewrite cross-bank
     ADDR12 calls to the trampoline; missing trampoline = hard error.
   - Unit test test_pack_banks.sh: **13/13** (placement, cross-bank detect,
     emit, assemble, rewrite-link, error-without-trampolines, flat path).

## Two integration follow-ups surfaced linking the real firmware (next)
A. **Common-region page-locality.** The hand-written runtime asm (regops/
   arith16/logic16/...) has page-local JCN/ISZ that I4004BranchRelax does NOT
   touch (it is a CodeGen pass; .s runtime is not relaxed). In the common region
   a runtime fn straddling a 256-byte boundary breaks (seen: __andhi3 PAGE8
   0x1f6 vs consumer page 2). FIX: apply --pad-pages semantics to the COMMON
   region only (keep each runtime/trampoline section whole in a page) while the
   banked firmware stays unpadded (relaxed). A per-region pad flag in
   _place_rom_banked.
B. **Cross-bank count vs geometry.** With common=2 KB the detector found 193
   cross-bank calls; with common=1.5 KB / bank=2.5 KB it found 0 (functions
   co-bank differently). Re-check under the final geometry after (A); the number
   drives how many trampolines/soft-stack depth the run needs. Also: crt0 must
   clear the __cur_bank cell (SRC 0x0F) to 0 at reset so the active-bank mirror
   matches the reset latch (bank 0); add that + a linker-defined __cur_bank_src.

## Then
2-3 bank chips call test (bank_test.rs ready) -> link whole firmware --banks and
report bank count/fill -> wire latch into chips run_image (oracle runner) + RTL
xl top -> 42-example chips run + compare.sh to 42/42 -> RTL.

## Validation
host 42/42 exact; lit 26/26; packer-bank unit 13/13; regops chips 10/10;
relax exec PASS; bank-latch primitive PASS. Every firmware fn <= 2,470 B.
