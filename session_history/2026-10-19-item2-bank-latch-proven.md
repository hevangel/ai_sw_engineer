# 2026-10-19 (cont.) — Item 2 started: ROM bank latch proven; packer placement is the core

Branch `system/busicom_141pf-on-4004`. No PR. timeout on every call.

## Done this run (committed + pushed)
- **Banking design + hardware-deviation doc** in `platform/memory_map.md`:
  latch on 4001 chip #3 output port; COMMON ROM #0-#3 (1 KiB, always bank 0,
  holds crt0/HAL/hot-rt/trampolines) + BANKED #4-#15 (3 KiB/bank); cross-bank
  call via a common-region trampoline that pushes the caller bank on the P7 soft
  stack, sets the latch, JMS target, restores; packer `--banks N` assigns
  functions to banks and rewrites cross-bank calls; both simulators honour the
  latch (chips adapter rebuilds roms[4..16]; RTL gates chip-select for #4-#15).
- **chips bank-latch harness** `llvm-i4004/rt/tests/bank_test.rs` +
  `run_bank_test.sh`: models the latch by watching b.roms[3].ports and rebuilding
  the banked chips from the selected bank (external/chips unmodified).
- **Latch primitive PROVEN on chips:** `SRC p(0x30); LDM 1; WRR` sets
  roms[3].ports = 1. The bank-switch write mechanism works.

## Core remaining item-2 work (the big piece)
The PACKER must become region/bank-aware. Today `i4004_pack.py` lays sections
contiguously with no notion of common vs banked, so a banked function lands in
the common region (observed: `target` at 0x01C) and every bank sees the same
code — the cross-bank test returns the common value, not the bank's. Needed:
`i4004_pack.py --banks N`:
  1. reserve the common region (chips #0-#3) for `.reset`/crt0/HAL/hot-rt +
     generated trampolines;
  2. assign each remaining function to a bank (call-graph aware; keep callee
     with its hot caller where it fits a bank's 3 KiB);
  3. emit a `__bankcall_<f>` trampoline per cross-bank-called function in the
     common region and rewrite cross-bank JMS/JUN to it;
  4. lay out common(1KiB) || bank_k_hi(3KiB) per bank; emit the image.
Then: wire the same latch into the RTL xl top (platform/rtl/busicom_141pf_xl.sv)
and the ORACLE runner (platform/scripts/chips_trace.rs, same roms[3] watch as
bank_test.rs), memory-test cross-bank calls on both, then items 3 (link whole
firmware --banks N, 42 examples on chips + compare.sh to 42/42) and 4 (RTL).

## Status
Item 1 (far-branch relaxation) DONE + chips-tested earlier this session. Item 2:
design + latch primitive done; packer bank placement is the next and largest
unit. Firmware links given enough ROM (43,552 B); size work (Tier A/B) continues
in parallel and, if it reaches <=4096 B, removes the need for banking entirely.
lit 26/26; regops chips 10/10; host 42/42 exact; relax exec test PASS.
