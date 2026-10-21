# 2026-10-19 — software-call convention items 1–4 landed & green on chips

Branch `system/busicom_141pf-on-4004`, worktree `ai_sw_engineer_run`.
Implemented the software-stack call convention end to end (abi.md 4.4.1/4.4.2)
as one coupled unit, items 1–4, all green.

## What landed (commits, oldest→newest)
- `4c770cf` **Item 1 — MC fixups/relocs.** `%lo8(sym)`/`%page(sym)` operand
  specifiers (I4004 `S_LO8`/`S_PAGE` MCSpecifierExpr) that materialise a code
  label's low 8 bits and page nibble into a FIM second word. New fixups
  `fixup_lo8`/`fixup_page4`, relocations `R_I4004_LO8=7`/`R_I4004_PAGE4=8`
  (ELF writer + packer resolve both), `MCAsmInfo::printSpecifierExpr` round-trip.
  `tests/mc/reloc_softcall.s`. MC lit 17/17.
- `27324b3` **Item 3 — backend CALLSOFT + soft returns.** `I4004ExpandPseudos`
  lowers a call to a compiled function to the software-call sequence — push a
  3-nibble return token (off.lo, off.hi, page) INLINE via scratch P0 + SSP(P7),
  then `jun callee` (no HW-stack level, no arg-register clobber) — and every
  compiled `RETpseudo` to `jun __soft_ret`. A call to a leaf rt/HAL helper
  (`__*`, mem*) keeps `jms`. Return site split to its own MBB, referenced with
  the `%lo8`/`%page` specifier via `MO_LO8`/`MO_PAGE` operand flags lowered in
  MCInstLower. lit 28/28; `tests/codegen/softcall.ll`.
- `eae0ee8` **Items 2 + 4 — packer synthesis + leaf assert + chips exec.**
  `i4004_pack.py --softcall`: reserve+emit a `jin p0` stub (0x31) at 0xFC on
  every page; synthesize `__soft_ret` (verified byte template) + `__srtab`
  (16 `jun <page stub>`) on a dedicated page; resolve the new relocations;
  `_assert_leaf_depth` rejects a JMS to a non-leaf (would overflow the 3-level
  HW stack). `tests/exec/run_softcall_exec.sh` runs llc-compiled CALLSOFT code
  on external/chips.
- `ce1232c` **Fix — `__soft_ret` must not clobber D1.** The page-dispatch JIN
  pointer moved from P2 (=D1, the return register) to P4 (R8:R9), page temp R10,
  so an i8/i16 return value survives the soft call.

## Proven on external/chips (`run_softcall_exec.sh`, all green)
- depth-4 call chain (4 soft frames > the 3-level HW stack) returns correctly
  through `__soft_ret → __srtab → per-page jin-p0 stub`;
- a callee-saved value survives a soft call (`g(2)=h(2)+2=14`);
- an **i16 return value** survives a 2-deep soft call (`wide(0x0102,0x0304)=0x0406`).
Host firmware `cargo test` 42/42, lit 28/28, leaf-depth assertion rejects a bad
JMS — all still green.

## Bugs found & fixed during bring-up
- `;` is a COMMENT in i4004 asm, not a statement separator — the hand-written
  reset stub's `src p7 ; ld r1 ; wrm` collapsed to just `src p7`. One instr/line.
- `__soft_ret`'s `jcn nc` must skip `iac;xch` and land on the `jin` (page-table
  carry path); an off-by-two first landed it on `iac`.
- `__soft_ret` clobbered P2 (=D1), corrupting i8/i16 returns; moved to P4.

## NOT done — the real next steps (items 5–6 gated on these)
1. **8-bit SSP carry (R15→R14).** The inline push only `inc r15`, so the soft
   stack is capped at the first 4002 register (16 nibbles = 5 three-nibble
   frames). The firmware call graph is 6+ deep, so the relink needs the SSP to
   carry the char nibble into R14 (walking reg/chip) on push AND pop. This is
   the gating fix for depth > 5.
2. **Recursion/large-frame value spill.** `sumdown` (recursive accumulate)
   returns a wrong value — a register-allocation spill bug around a value live
   across a self-call, independent of the soft-call control flow.
3. Then **item 2 banked synthesis**: extend `--softcall` to the `--banks` path
   (stub per bank page, `__soft_ret` latch write from the token's bank nibble,
   DELETE the old `__bankcall_*` JMS trampolines) — the current synthesis is the
   flat path only.
4. Then **items 5–6**: wire `build_image.sh` to pass `--softcall`, relink the
   whole firmware, wire the bank latch into `platform/scripts/chips_trace.rs`,
   run the 42 examples vs `firmware/oracle/rom_port_trace.txt`.
