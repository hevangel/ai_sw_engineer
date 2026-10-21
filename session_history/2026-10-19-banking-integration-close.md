# 2026-10-19 (cont.) — banking integration: common page-pad, crt0, mul split; link very close

Branch `system/busicom_141pf-on-4004`. No PR. timeout on every call; host 42/42
exact at every commit.

## Done this run (committed + pushed)
- **(A) common-region page-pad**: i4004_pack.py --banks now keeps each COMMON
  code section (hand-written runtime/HAL/trampoline asm, page-local JCN/ISZ not
  relaxed) whole inside a 256-byte page; banked firmware stays unpadded. Fixes
  the __andhi3 PAGE8 page-cross.
- **(B) crt0** clears the active-bank mirror __cur_bank (SRC 0x0F) to 0 at reset.
- **mul split**: extracted mul_add_row inline(never) from Num::mul (2470 B);
  with earlier Tier-B, every firmware fn is now <= ~2,340 B.
- Geometry: common=0x680 (1664 B, runtime ~1550 fits), bank=0x980 (2432 B,
  fits the 2340-byte do_mul_div_equals), view = 4096 (16 chips).

## Banked link status: VERY CLOSE
Pass 1 (emit trampolines) succeeds; pass 2 (rewrite-link) now resolves nearly
all relocations and fails only on ONE residual:
  do_mul_div_equals+0x57f: PAGE8 target 0xc02 (page 12) vs consumer page 11.
ROOT CAUSE: I4004BranchRelax turns `JCN cc,Dest` into `JCN !cc,.Lcont ; JUN
Dest`. The far target is handled by the JUN, but the short `JCN !cc,.Lcont`
(.Lcont = +3 bytes) still uses JCN's 8-bit PAGE-LOCAL field, so when that JCN
sits in the last 1-3 bytes of a 256-byte page its +3 target lands on the next
page -> illegal page-local reloc.
FIX (next, bounded): ensure a relaxed JCN never sits within 3 bytes of a page
end. Either (a) a packer micro-pad: when placing banked sections, pad (NOP, 0x00)
so no `JCN` opcode falls at page offset >= 0xFD; or (b) in I4004BranchRelax mark
the JCN+JUN as an un-splittable 5-byte bundle and have the packer page-align the
bundle. (a) is simplest and local to _place_rom_banked / _apply.

Note: pass 1 reported "0 cross-bank trampolines" under this geometry (30 banks,
greedy first-fit co-located everything's callers+callees, or calls go to common
runtime) -- confirm by reading the .map once the link completes; if genuinely 0,
no trampolines are needed and the latch is only exercised by the bank the
packer puts each fn in (still needs the run_image latch wiring for >bank-0 fns).

## Then
chips run_image/oracle-runner latch wiring (platform/scripts/chips_trace.rs,
same roms[3] watch as bank_test.rs) + RTL xl top latch -> 42-example chips run +
compare.sh, record first divergence -> RTL.

## Validation
host 42/42 exact; packer-bank unit 13/13; lit 26/26; regops chips 10/10; relax
exec PASS; bank-latch primitive PASS. Every firmware fn <= ~2,340 B.
