# 2026-10-19 — items 5 done (banked relink), 6 localized (cross-bank latch)

Branch `system/busicom_141pf-on-4004`, worktree `ai_sw_engineer_run`. Resumed
after a backend 5xx at `d1d9875` (soft-call items 1–4 green). This session
unblocked and completed the whole-firmware banked relink (item 5) and localized
the one remaining mechanism for the 42-example run (item 6).

## Landed (commits, oldest→newest)
- `27a1aaf` **8-bit SSP pointer → unbounded soft-call depth.** The push/pop now
  advance P7 (R14:R15) as a full 8-bit pointer (carry R15→R14 via the
  `clc; ldm 1; add r15; xch r15; ldm 0; add r14; xch r14` trick), so the soft
  stack spans the whole 4002 arena, not one 16-char register. Two P0-only leaf
  push helpers `__push_off`/`__push_pg` (so i16 args in D1/D2 survive — the
  earlier single helper staged the page in P4 and clobbered the D2 i16 arg).
  Backend CALLSOFT: `fim p0,%lo8; jms __push_off; fim p0,%page; jms __push_pg;
  jun callee`. Chips exec now proves a **depth-8** chain + value preservation +
  i16 return; lit 28/28.
- `42bd931` **crt0 soft-enters main; build_image passes `--softcall`.** Confirmed
  the whole firmware compiles + links-flat (48,801 B) + passes the leaf-depth
  assertion through the soft-call pipeline.
- `985af62` **banked `--softcall` synthesis.** The banked path synthesizes the
  return dispatch in the COMMON region (mapped in every bank) and the per-page
  `jin p0` stub into every page of every bank (`_synth_softcall_banked` +
  `_write_softcall`). Found the real relink blocker: 12-bit PC ⇒ common+bank ≤
  4096, and 2 functions exceeded the window.
- `1a9bcfd`, `3e3c473` **four `#[inline(never)]` firmware splits** (host 42/42
  exact): `bcd::Num::divide` 2401→1939 (MSD scan into `assemble_quotient`),
  `Calculator::do_mul_div_equals` (→`compute_mul_div`), `driver_printer
  sector_masks` (→`numeric_masks`), `Calculator::dispatch` (→`dispatch_key`).
  Now 0 of 64 functions > 2048 B.
- `be185dd` **report: item 5 DONE.** `build_image.sh --board xl BANKS=25
  COMMON_BYTES=0x800 BANK_BYTES=0x800` links the whole firmware into a
  **53,248-byte banked image (common 0x800 + 25 × 0x800), banks ~99 % full**.

## Item 6 — the one remaining mechanism (precise design)
Running the 42 examples needs the cross-bank FORWARD latch switch, currently
unimplemented (a call to a callee in another bank reaches the right PC in the
WRONG bank because nothing sets the ROM bank latch before the `jun`). Design:
1. **Token → 4 nibbles** (off.lo, off.hi, page, **bank**). `__push_pg` also
   pushes the current bank read from the `__cur_bank` RAM mirror (chip0/reg0
   char 0x0F, already cleared by crt0).
2. **`__soft_ret`** pops the 4th (bank) nibble and writes the ROM bank latch
   (4001 chip-3 output port: `fim p0,0x30; src p0; ldm <bank>; wrr`) + updates
   `__cur_bank`, restoring the caller's bank before dispatch.
3. **Packer** routes each cross-bank `jun callee` through a per-(bank,callee)
   common set-latch stub `__setbank<B>_<callee>` (set latch=B, update
   `__cur_bank`, `jun callee`) — inserting instructions in place would shift
   addresses, so a common stub is cleaner. The old `__bankcall_*` JMS
   trampolines are deleted.
4. **chips_trace.rs** must split the 53 KB image into common + per-bank windows,
   start on bank 0, and on a latch write (`roms[3].ports`) rebuild `roms[4..]`
   from the selected bank — the exact pattern already proven in
   `llvm-i4004/rt/tests/bank_test.rs`.
Only then: `run_image.sh secondary --board xl` over the 42 examples +
`compare.sh` vs `firmware/oracle/rom_port_trace.txt`.

## Also open (separate from banking)
A recursion/large-frame value-spill bug in register allocation (`sumdown`
returns a wrong value) — independent of the soft-call control flow.

## Green at session end
lit 28/28, soft-call chips exec (depth-8 + i16 + value-preservation), host
firmware `cargo test` 42/42 exact, leaf-depth assertion rejects a bad JMS,
whole firmware relinks to a 25-bank image.
