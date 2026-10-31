# MC68000 core — implementation plan

Goal: a `no_std` MC68000 core validated to **zero mismatches** against
the SingleStepTests 680x0 vector suite (1,000,060 vectors), serving as
the instruction-level oracle for the ai_hw_engineer `motorola_68000`
RTL and the executor for the 68k LLVM-stack gates.

## Phases

1. **Skeleton + harness** (done): `step()` instruction-level core,
   flat logging bus, replay + probe test harnesses reading the
   compiled vector streams (`m68k_tests.bin`, `m68k_index.bin`,
   `m68k_final.bin`, `m68k_prefetch.bin`, `m68k_names.json`).
2. **Canonical decode sweep** (done): every instruction group decoded,
   address modes 0-7, exception entry, privilege checks.
3. **Oracle fitting** (in progress, 898,188/1,000,060): fit the core
   to the captured silicon row by row. The suite deviates from the
   PRM in decode boundaries (MOVEP/bit ops, MUL destination register,
   CHK rows), flag corners (NBCD/ABCD/SBCD decimal windows, MUL V=0,
   GT) and fault frames (SSW labels, IIRD latching, post-instruction
   fetch faults, BSR continuation push). Fitted rules are recorded in
   `emulator/spec/singlesteptests.md` with the fitting method.
4. **Remaining classes** (pending):
   - to-SR family: model the suite's non-zero reference memory or
     derive the loaded values from the stream.
   - RTS/RTR fault-frame trailing words.
   - CMPA/MOVEM/DBcc/Scc/DIVU/MOVEP/SBCD/ABCD residues.
5. **Cleanup + merge** (pending): remove `tests/probe.rs` (scratch),
   `cargo fmt`/`cargo clippy`, run the whole test suite, then merge
   PR #269 (blocked on zero mismatches by the PR's own statement).

## Rules

- The oracle decides; the manual is a hint. Every deviation is written
  down with the evidence that produced it.
- No submodule pin updates, no website changes from this task.
- Commits on `chip/motorola-68000`; PR #269 is the review vehicle.
