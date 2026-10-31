# llvm-m6800 implementation plan (stage 1)

For hevangel/ai_engineer#61. Follows the proven llvm-i4004/i8008 route
(overlay onto the pinned LLVM tree, Docker-built tools, execution tests
against the project's MC6800 RTL). Nothing below is implemented; each
milestone lands with its own tests and report.

## M0 — Specification freeze (current stage)

- [x] Upstream audit ([../spec/audit_upstream.md](../spec/audit_upstream.md)).
- [x] Compiler contract ([../spec/compiler_profile.md](../spec/compiler_profile.md)).
- [ ] `spec/isa.md`: the 72-instruction documented set with per-mode
      encodings, derived from the M68PRM(D) opcode matrix (Table 3-1) and
      Appendix A flag definitions; MC tests encode every opcode.
- [ ] `spec/abi.md`: pair model for i16 (IX-based frames vs software
      frames), calling convention, direct-page allocation policy — before
      any TableGen.
- [ ] `rust/m6800-none.json` target JSON design (big-endian, 16-bit
      pointers at 8-bit alignment, build-std core, soft-float).

## M1 — MC layer

- Encoder/disassembler for the documented set; asm syntax (Motorola
  mnemonics, matching the PRM and the assemblers' common syntax);
  relocations: 8-bit relative branches (BR*), direct/extended absolute,
  indexed offsets.
- lit tests: one CHECK per opcode, encode/decode round-trips, relocation
  tests; independent decoder cross-check (vasm 6800 or the vendored
  hakanrw test assembly as ground truth).
- Execution probe: a hand-assembled program runs on the ai_hw_engineer
  MC6800 RTL harness before any CodeGen exists.

## M2 — CodeGen

- SelectionDAG lowering: i8 native in A/B; i16/i32/pointers expanded to
  byte pairs with carry-chain lowering (the llvm-i8008 pair model, but
  with hardware push/pull available); direct-page globals as a cost-model
  class; big-endian everywhere.
- Calling convention per the frozen `spec/abi.md`; IX as frame pointer is
  in tension with IX-as-address-register — the allocation strategy is the
  6800-specific risk, resolved at M2 with lit tests.
- `exec_fn`-style execution tests: llc-compiled functions run on the
  MC6800 RTL, compared against host results.

## M3 — Runtime, linker and packaging

- `rt/`: crt0 per the contract; `libm6800rt` (pair add/sub/shift/mul via
  shift-add, div, memcpy/memset).
- Linker: project packer first (S-record + flat binary output, ROM budget
  gate), `ld.lld` relocation support as a follow-up evaluation.
- Regression: whole-program gate re-runs the 50-program third-party suite
  against RTL with our image in place of hand-assembled code where a
  consumer title exists.

## M4 — Rust profile (issue #61's deliverable)

- Pinned rustc + `m6800-none.json`; representative no_std smoke programs
  compiled, opcode-verified independently, executed on the MC6800 RTL with
  recorded expected output; regression suite + report per the acceptance
  list.

## Risks / open questions

- One index register: frame-pointer vs address-register conflict dominates
  M2; static frames (llvm-i4004 style) may win over dynamic frames.
- No hardware multiply: i16 multiplication is a libcall from day one;
  performance is a non-goal (correctness first).
- Big-endian 16-bit pointer lowering must be exercised from the first
  probes (endianness bugs are silent until pointers dereference).
