# Packer page relaxation: five ways a relaxed branch landed in the wrong place

- Area: llvm-i4004 (`asm/i4004_pack.py`: `_page_align_section`, `_splice_relax`)
- Found by: the BCD primitive execution test (`system/busicom_141pf/firmware/
  scripts/exec_num.py`), which runs each hand-written routine of `asm/num.s` on
  external/chips against its Rust twin. The packer's own tests and the lit/MC
  suites were green throughout.

## What broke

A JCN/ISZ whose target is on another 256-byte page is rewritten by the packer
into a page-independent form (`JCN !cc, .+4 ; JUN T`, or `ISZ Rn, .+4 ; JUN
cont ; JUN T`), or the instruction is padded onto the next page with NOPs. Three
defects in that code made the rewritten code branch to the wrong address:

1. **Local-label references were not shifted.** The assembler refers to a
   `.L` label as `<section symbol> + addend`. When relaxation or padding grew a
   section, the packer moved relocation sites and named symbols but left these
   addends alone, so every later branch to a local label landed `grow` bytes
   early. (`__n_div` returned garbage.)
2. **The relaxed skip operand was section-relative.** The near `JCN !cc, .+4`
   operand was written as the section offset of `.+4` instead of its absolute
   page-local address, so the skip only worked in sections placed at a page
   boundary. (`__n_div` again, once bug 1 was fixed.)
3. **The skip target could fall off the page.** The fit check accepted a
   relaxed block when its last byte stayed on the page (`op + blk - 1 < 256`).
   A JCN at page offset 0xFC therefore became `JCN !cc, .+4` with `.+4` = 0x100,
   and the 8-bit operand 0x00 sent it to offset 0 of its *own* page. (`__n_mul`
   ran off into empty ROM and restarted the driver; `__n_add` returned junk.)

   Two more surfaced when the packer began ordering sections to fill pages
   (issue #18), which put code at new offsets:
4. **A JCN in the last two bytes of a page.** The 4004 takes a JCN/ISZ target
   on the page of the *next* instruction, so one at 0xFE/0xFF jumps into the
   following page. The check compared the target with the instruction's own
   page and passed it. (`__n_compute` ran off; `__n_round` failed.)
5. **A relaxed skip byte went stale.** The skip operand was written as the
   absolute low byte of `.+4`, computed from the section's base when it was
   relaxed. A later relaxation of an *earlier* branch in the same function
   (2 bytes inserted pushed that branch's target across the page) moved the
   block by 2 bytes, and the byte pointed into the JUN. (`__n_round`: a
   `jcn z, 6f` in `__leaf_n_shr` skipped to the wrong place.)

## Root cause and why our tests missed it

The relaxation was written for compiler output, and until issue #18 nothing
exercised it at scale. Compiled functions branched to blocks the packer saw as
named symbols, and the lit tests stop at llc's output, before packing.
`exec_cmp.py`/`exec_nibble.py` run packed code, but their routines are short
and start on a page, so neither the addend path nor offset 0xFC came up. The
hand-written routines in `asm/num.s` are the first long, local-label-heavy
code placed at arbitrary offsets, and they hit all five.

## Fix and re-verification

- `_shift_section()` now moves relocation sites, named symbols and every
  section-relative addend at or past the insertion point. Both the padding
  path and `_splice_relax` use it, and the far-JUN relocation that a
  relaxation appends gets the same adjustment.
- Relaxed operands are absolute page-local addresses, not section offsets
  (now through relocations, see below).
- A relaxed block is accepted only when `.+4` itself is on the page
  (`op_pagerel + 4 < 256`). Otherwise the op is padded to the next page first.
- A JCN/ISZ is checked against its landing page (the page of the next
  instruction); one at 0xFE/0xFF is padded off it.
- The skip operand of a relaxed block is a PAGE8 relocation to a local symbol
  (`_add_skip_reloc`), resolved after the last splice, so blocks may move
  freely (and the page-filling order may be recomputed on every pass).
- `exec_num.py`: 4,200/4,200 on chips after fixes 1-3 (21 routines × 200
  random cases, including the whole-operation entries); 6,900/6,900 after
  fixes 4-5, under both the input order and the page-filling order.

## Prevention

- Every hand-written routine is linked by the real packer and executed on
  chips against its twin (`exec_num.py`). That test is the packer's real
  regression test, because it places code at arbitrary page offsets.
- Rule: a packer change that rewrites code is not verified by lit/MC tests.
  It needs an execution test whose code lands at every page offset class
  (start of page, mid-page, last four bytes).
