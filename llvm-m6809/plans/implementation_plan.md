# llvm-m6809 implementation plan (stage 1)

For hevangel/ai_engineer#62. Follows the proven llvm-i4004/i8008 route
(overlay onto the pinned LLVM tree, Docker-built tools, execution tests
against the project's MC6809 RTL). Nothing below is implemented; each
milestone lands with its own tests and report.

## M0 — Specification freeze (current stage)

- [x] Upstream audit ([../spec/audit_upstream.md](../spec/audit_upstream.md)).
- [x] Compiler contract ([../spec/compiler_profile.md](../spec/compiler_profile.md)).
- [ ] `spec/isa.md`: the documented instruction set (pages 1/2/3) with the
      full indexed-postbyte encoding table, per the M6809 Programming
      Manual; MC tests encode every opcode × addressing form.
- [ ] `spec/abi.md`: calling convention (gcc6809-anchored), DP policy,
      U-vs-S frame model, PSHS/PULS save-set lowering — before TableGen.
- [ ] `rust/m6809-none.json` target JSON design (big-endian, 16-bit
      pointers at 8-bit alignment, build-std core, soft-float).

## M1 — MC layer

- Encoder/disassembler with the indexed postbyte table as the centerpiece
  (canonical-form encoding + minimal-form selection is a codegen-time
  choice; MC emits both from the table).
- Relocations: branch short/long, direct-page, extended absolute,
  OS-9-reference PC-relative forms.
- lit tests per opcode/form; independent decoder cross-check against
  LWTOOLS `lwasm` output (actively maintained, 2025).
- Execution probe: hand-assembled program on the ai_hw_engineer MC6809
  RTL harness before any CodeGen exists.

## M2 — CodeGen

- i8 native (A/B); i16/pointers in D/X/Y; i32+ expanded; big-endian.
- Indexed-mode selection: the postbyte table maps naturally onto
  addressing-mode matchers; LEA-based address arithmetic (LEAX offset,X)
  is the 6809's strength-reduction lever and the main codegen quality
  differentiator vs the 6800.
- Frames: `leas -k,s` prologues + PSHS/PULS saves; DP policy per the
  frozen ABI.
- `exec_fn`-style execution tests against the MC6809 RTL.

## M3 — Runtime, linker and packaging

- `rt/`: crt0 per the contract; `libm6809rt` (divide, wide shifts,
  memcpy/memset).
- Linker: project packer emitting S-records and DECB binaries first
  (budget gates), OS-9 module format with CRC as a follow-up; lwlink as
  the format cross-check.
- Regression: re-verify the reyco2000 suites still pass with our images
  loaded alongside (no RTL modification), then a consumer title on the
  RTL when a machine profile lands.

## M4 — Rust profile (issue #62's deliverable)

- Pinned rustc + `m6809-none.json`; representative no_std smoke programs
  compiled, opcode-verified independently (py65-style table from LWTOOLS),
  executed on the MC6809 RTL with recorded expected output; regression
  suite + report per the acceptance list.

## Risks / open questions

- The indexed-postbyte table is large (every mode × index register ×
  indirect) — the MC-layer cost is the 6809-specific multiplier; the
  canonical-encoding table must be generated and lit-tested, not
  hand-maintained.
- DP policy (fixed page vs allocatable base) changes CodeGen shape; decide
  at M0 with measurements, not by taste.
- FIRQ's minimal frame makes it attractive for a future runtime's fast
  path, but interrupt plumbing is a machine-profile concern, not the
  compiler's — keep it out of M2.
