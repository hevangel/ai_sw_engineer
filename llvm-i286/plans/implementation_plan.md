# llvm-i286 implementation plan (stage 1)

For hevangel/ai_sw_engineer#66; backend qualification is coordinated on the
roadmap as hevangel/ai_engineer#63. The plan mirrors the proven llvm-i4004
route (overlay onto the pinned LLVM tree, Docker-built tools, execution
tests against independent simulators). Nothing below is implemented yet;
each milestone lands with its own tests and report.

## M0 — Specification freeze (current stage)

- [x] Upstream audit ([../spec/audit_upstream.md](../spec/audit_upstream.md)).
- [x] Machine/compiler contract ([../spec/machine_profile.md](../spec/machine_profile.md)).
- [ ] `spec/isa.md`: the exact 8086+186+286 instruction subset with
      encodings (per mode milestone), derived from the iAPX 286 Programmer's
      Reference; explicit forbidden-later-instruction list enforced by MC
      tests.
- [ ] `spec/abi.md`: data layout, register classes (i8/i16 native, i32
      pairs), near/far address spaces, calling convention, stack rules —
      before any TableGen, like llvm-i4004.
- [ ] `rust/i286-none.json` target JSON design (build-std core; soft-float).

## M1 — MC layer

- Encoder/disassembler for the subset; asm syntax spec
  (Intel-flavored, matching the reference manual); relocations for
  segment:relative addressing (R_386-style 16-bit counterparts: segment,
  offset16, 16:16 far pointers).
- lit tests: one CHECK per opcode, encode/decode round-trips, relocation
  tests; disassembler cross-check against an independent decoder (binutils
  objdump built for ia16, or capstone) as the external oracle.
- Execution probe: hand-assembled image runs on an independent 286-class
  emulator before any CodeGen exists.

## M2 — CodeGen

- SelectionDAG lowering for i8/i16 (native) and i32/i64 (expanded pairs);
  16:16 segmentation via address-space pointer lowering; no 386 fallback —
  aisel patterns must reject anything outside the frozen subset (tests assert
  rejection).
- Calling convention per `spec/abi.md`; static or dynamic frames; varargs
  policy deferred until a consumer needs it.
- lit selection-pattern tests + `exec_fn`-style chips-style execution tests
  against the independent emulator, like llvm-i4004's `scripts/exec_fn.py`.

## M3 — Runtime, linker and packaging

- `rt/`: crt0 (real-mode entry, stack setup), libi286rt (i32/i64 helpers,
  memcpy/memset), HAL contract for the PC AT (BIOS doors vs bare-metal).
- Linker: ld.lld is not aware of i286 relocations — follow the i4004
  precedent with a project packer first (MZ/COM/flat/option-ROM layout,
  checksums, budget gates), and evaluate an lld backend patch as a follow-up.
- Relocation/format tests for each package format in
  [../spec/machine_profile.md](../spec/machine_profile.md).

## M4 — Rust profile (issue #66's deliverable)

- Pinned rustc + `i286-none.json`; smoke Rust programs (no_std) compiled to
  IR, lowered, linked, executed on the qualified independent emulator with
  expected output recorded; regression suite + report per the issue's
  acceptance list.

## Risks / open questions

- SelectionDAG legality for a strict 286 subset is the largest unknown; the
  i4004 experience says constant lowering dominates the effort.
- 16:16 far pointers as LLVM address spaces need the same datalayout care
  the 4004 needed for its 16-bit pointers.
- Protected-mode milestone depends on sourced software needing it; do not
  build GDT/LDT machinery speculatively.
