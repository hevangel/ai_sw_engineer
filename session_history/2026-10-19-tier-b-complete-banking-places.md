# 2026-10-19 (cont.) — Tier-B split COMPLETE (every fn <= 2.5 KB); banking places; trampolines next

Branch `system/busicom_141pf-on-4004`. No PR. timeout on every call; host 42/42
exact at every commit.

## Done this run (committed + pushed)
**Tier-B: every compiled firmware function is now <= 2,470 bytes** (was: run
8,276 / divide 4,815 / sqrt_at 3,906 / print 3,497). Done by #[inline(never)]:
- lib.rs: press, dispatch, and every Calculator dispatch method
  (add_sub/mul_div_key/equals/percent/memory_*/sqrt/compute*/digit/clear/...) ->
  run/dispatch no longer absorb the whole program by monomorphised inlining.
- driver_keyboard scan_pass, driver_lamps write_lamps, driver_printer print +
  sector_masks + shift_hammer_word.
- bcd.rs: extracted restoring_divide, assemble_quotient, build_div_operands from
  Num::divide; marked isqrt_digits + trial_product inline(never) (extracts
  sqrt_at).
Result: firmware fits a common(1.5 KB) + 2.5 KB-bank layout. Total .text ~45 KB.

## Banking placement VALIDATED
`i4004_pack.py --banks 24 --common-bytes 0x600 --bank-bytes 0xA00` now PLACES
the whole firmware (functions spread across banks 0-17) and reports exactly
**193 cross-bank ADDR12 calls** that need trampolines. So: Tier-B removed the
"function bigger than a bank" blocker; the only thing between here and a linked
banked image is trampoline synthesis.

## Next step (the remaining item-2 core)
Trampoline synthesis in i4004_pack.py: for each cross-bank-called callee,
generate `__bankcall_<callee>` in the common region (push caller bank on the P7
soft stack via __push_byte, write latch = SRC chip3/LDM bank/WRR, JMS callee,
restore latch, pop, BBL - preserving the P1 return) and rewrite the 193
cross-bank JMS relocations to the trampoline. Then: 2-3 bank chips call test
(the bank_test.rs harness is ready); link whole firmware --banks and report bank
count/fill; wire the latch into the RTL xl top + the oracle runner; run the 42
examples on chips + compare.sh to 42/42; then RTL.

## Validation
host cargo test 42/42 exact; lit 26/26; regops chips 10/10; relax exec PASS;
bank-latch primitive PASS; packer-bank unit 8/8. Every firmware fn <= 2,470 B.
