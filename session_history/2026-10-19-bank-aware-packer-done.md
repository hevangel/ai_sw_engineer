# 2026-10-19 (cont.) — bank-aware packer DONE (8/8 unit); Tier-B split is the prerequisite

Branch `system/busicom_141pf-on-4004`. No PR. timeout on every call; host 42/42
exact; lit 26/26 throughout.

## Done this run (committed + pushed)
**`i4004_pack.py --banks N` — bank-aware linker (item-2 core).**
- Classifies ROM code into COMMON (.reset/.rodata + runtime/HAL/trampoline
  helpers by name: __*, _start, reg_*, memset/memcpy/memmove) vs BANKED (the
  firmware's mangled Rust functions).
- Common region [0, --common-bytes) identical in every bank; banked functions
  greedily bin-packed into N banks' [--common-bytes, +--bank-bytes).
- Emits common || bank_0 || ... || bank_{N-1}; .map lists each function's bank +
  per-bank fill.
- DETECTS cross-bank ADDR12 calls and errors (does not silently miscompile;
  trampoline synthesis is the next step).
- Default (flat, no --banks) path untouched.
- Unit test `llvm-i4004/asm/tests/test_pack_banks.sh`: **8/8** (classification,
  placement, cross-bank detection, image size, map, flat path).

## Prerequisite the packer surfaced (must do before banking can place the firmware)
Linking the real firmware with --banks reports, accurately:
1. runtime+crt0+HAL common set is ~1.4 KB -> common region must be >= 2 KB
   (chips 0-7, so banks are chips 8-15 = 2 KB each) -- a config choice.
2. **A single function exceeds a bank: `target::run` = 8,276 B** (bigger than
   even a full 4 KB bank). `divide` ~5 KB and `sqrt_at` ~4.3 KB are also over.
   => **Tier-B `#[inline(never)]` splitting of run/divide/sqrt_at into
   sub-bank-sized pieces is a hard PREREQUISITE** for banking (and for any
   <=4 KB layout at all). This is the next unit: split those functions in
   firmware (keeping host tests 42/42 exact), re-measure, then banking can place
   them, then synthesise the cross-bank trampolines, wire the latch into the RTL
   xl top + the oracle runner, and run the 42 examples.

## Status chain this session
Item 1 far-branch relaxation: DONE + chips-tested. Item 2: design + latch
primitive proven on chips + bank-aware packer done & unit-tested; remaining =
Tier-B splitting (prereq) -> trampolines -> RTL latch + oracle-runner latch ->
42-example chips run -> RTL. Firmware size 42,114 B; links flat given >4 KB ROM.
