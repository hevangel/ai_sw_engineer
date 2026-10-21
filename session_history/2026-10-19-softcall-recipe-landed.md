# 2026-10-19 (cont.) — soft-call integration recipe landed (verified encodings); items 1-3 scoped as one coupled unit

Branch `system/busicom_141pf-on-4004`. No PR. host 42/42 exact. HEAD 841e12b.

## Landed this run (committed + pushed)
- 841e12b  report/final_report.md: EXACT soft-call integration recipe for items
           1-3 with llvm-mc-VERIFIED byte encodings (jin p0 = 0x31;
           jun 0xKFC = 0x4K 0xFC; fim p1,imm8 = 0x22 imm8). Records that 40/43
           firmware fns are non-leaf, so nearly every call site + return is
           affected.
  Plus prior this chain: softcall_core.s (depth-4) + softcall_mp.s (cross-page)
  both green on chips; abi 4.4.1/4.4.2; rt/softcall.s __push_ret (13 B).

## Why items 1-3 are ONE coupled unit (not finishable+green in a single turn)
- Packer synthesis (1) needs per-page RESERVATION of the jin-p0 stub byte
  (0xFC) + the __soft_ret/__srtab block, done BEFORE section placement in
  _place_rom/_place_rom_banked (bump real sections past the reserved window,
  like the page-align pass). Emitting __soft_ret as hand-encoded bytes is
  brittle; the clean form is packer-emitted bytes with the known &__srtab
  (the proven softcall_mp.s dispatch sequence), NOT symbolic nibble math.
- Backend (2) needs TWO NEW MC fixups that do not exist yet:
  R_I4004_IMM8_LO (code-label low 8 bits -> FIM) and R_I4004_IMM4_PAGE
  (code-label page nibble -> FIM), wired through MCCodeEmitter + AsmBackend +
  packer _patch. This is the main new MC work and forces an LLVM rebuild.
- (1) and (2) are mutually dependent (the packer must know what the backend
  emits at a call site; the backend's FIM operands need the new relocs the
  packer resolves). Landing one without the other leaves no runnable path and
  risks a non-building toolchain across a resume.

## Exact next actions (resume here)
1. Add R_I4004_IMM8_LO + R_I4004_IMM4_PAGE: enum in isa/abi reloc table;
   MCCodeEmitter fixup kinds; AsmBackend applyFixup; packer _patch cases.
2. ExpandPseudos: CALL -> CALLSOFT (fim p1,<lo8 .Lret>; fim <pagereg>,<page
   .Lret>; jms __push_ret; jun callee; .Lret:); RETpseudo -> `jun __soft_ret`
   for non-leaf MFs (leaf = contains no call -> keep BBL). JOBS=16 build_llvm.sh.
3. Packer: self.soft_calls; reserve 0xFC stub + __soft_ret/__srtab block in
   placement; _synthesize_softret() writes jin-p0 per page, __srtab[k]=jun
   0xKFC, __soft_ret = proven dispatch bytes; register __soft_ret/__push_ret/
   __srtab linker-defined. Banking: latch write in __soft_ret, DELETE the 41
   __bankcall_* trampolines.
4. Packer leaf-depth<=3 assertion over the JMS-reachable subgraph.
5. chips tests: depth-8 plain + cross-bank; recursion depth 5; return values.
6. Then (5)-(6): relink firmware --banks; 2-3 bank test; latch into
   platform/scripts/chips_trace.rs; 42-example run + compare.sh; first divergence.

## Validation now
softcall_core chips depth-4 PASS; softcall_mp cross-page PASS; __push_ret
assembles; byte encodings verified. host 42/42 exact; packer unit 15/15; lit
26/26; regops chips 10/10.

## Unavailable this session
MCP @kirocrew-computer (declared, not configured) — reported, continued.
