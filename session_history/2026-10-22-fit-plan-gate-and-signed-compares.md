# 2026-10-22 — Fit plan (issue #18): size gate, signed compares, frame slots

- Branch: `llvm/i4004-fit` (from `main` fcc21e4)
- Systems: `llvm-i4004`, `system/busicom_141pf/firmware`
- Input: issue #18 "Fable code review" (review + 14-task engineering plan).
  This session did plan tasks 1 (P1 size gate, P2 scoreboard, P4 stop rule)
  and 2 (A2 signed compares).

## Done

1. **A2 signed compares.** `mapCC` now emits `I4004CC::SLT/SGE` (SGT/SLE by
   operand swap); `emitBrCC` flips bit 3 of both top nibbles with
   `RAL ; CMC ; RAR` (CY-neutral) so the unsigned borrow chain gives the signed
   order. +13 B per signed compare.
2. **Frame-slot aliasing (found while testing 1).** `addExternalSymbol(Sym,
   Off)` passes `Off` as target flags; every spill/reload/LEA_FI hit
   `__frame_<fn>+0`. New `frameSymOperand()` sets a real operand offset.
3. **`scripts/exec_cmp.py`**: ten predicates x {zext value, branch} x {i8, i16}
   over a sign-boundary matrix, whole llc output assembled behind a driver,
   executed on external/chips. Baseline llc 3,744/5,540; fixed 5,540/5,540.
   FileCheck: `compare_signed.ll`, `frame_offsets.ll`.
4. **`run_tests.sh` repaired** (it was red on main for reasons unrelated to
   this work): `crosscheck_disasm.py` CRLF shebang (now run via `python3`) and
   `cargo run` without `--bin` (two binaries now); `exec_fn.py`/`exec_i16.py`
   cut the body at `bbl` but compiled functions return via `jun __soft_ret`
   (was 6/20 and 5/20; now 20/20, 20/20).
5. **P1 size gate** in `gen_worklist.py` (whole module, `-function-sections`,
   sum of `.text*`; fails on ROM_BUDGET=4096 / FN_MAX=1024, `SIZE_GATE=report`
   to downgrade), run last in `run_tests.sh`; v0 names rendered readably.
6. **P2** `firmware/report/size_history.md`; AGENTS.md checklist item + size-gate
   rule; **P4** stop criteria in `plans/implementation_plan.md`.
7. Post-mortems: `failure_notes/2026-10-22-llvm-i4004-signed-compares-as-unsigned.md`,
   `failure_notes/2026-10-22-llvm-i4004-frame-slots-alias.md`.

## Measurements

| | .text B | largest fn B | jms | bitwise libcalls |
|---|---|---|---|---|
| baseline (main llc) | 56,945 | 1,790 | 653 | 93 |
| after A2 + frame fix | 57,010 | 1,790 | 653 | 93 |

Host `cargo test`: 12 unit + manual_examples + port_level + port_trace green.
`run_tests.sh`: lit 17/17 MC + 14/14 codegen, disasm crosscheck clean, exec
20/20 + 20/20 + 5,540/5,540, size gate FAIL (13.9x) — expected.

## Not done / next

- Task 3 (A1 native nibble patterns) is next: 93 bitwise libcalls in the asm.
- BRZ "low nibble only" (RC-8) is not live: BRZ/BRNZ have no ISel pattern.
- The 56,945 B baseline differs from the report's 42,114 B (older IR); the
  scoreboard re-measures from current sources.

## Continuation — task 3, A1 native nibble ops

- New nodes/pseudos `LOGICI`, `SHIFTI`, `NIBSEL` (i8/i16), selected in
  `LowerBitwise` before the libcall fallback; immediate encodings in
  `I4004ISelLowering.h` (`I4004Nib`). `computeKnownBitsForTargetNode` covers all
  three, so later ops still see known-zero nibbles.
- Expansion (`I4004ExpandPseudos`): whole-nibble masks are copies, `LDM`/`FIM`
  constants or `CMA`; partial masks edit bits while they sit in CY (the
  5-step `RAL` ring of ACC+CY, or `RAL;op;RAR` / `RAR;op;RAL` for a lone bit,
  `RAR;TCC` for `& 1`); constant shifts are nibble moves plus CY rotate chains,
  inline when <= 20 B (else the libcall stays); `ashr x, width-1` is a sign
  fill. NIBSEL looks through a constant AND that keeps the nibbles it takes, so
  `(b & 0xF0) | v` is `ld; xch`.
- Tests: `tests/codegen/nibble_ops.ll`; `scripts/exec_nibble.py` (constant
  masks, every constant shift amount, NIBSEL shapes; i8+i16) 5,520/5,520 on
  chips, wired into `run_tests.sh`.
- Firmware: `.text` 57,010 -> 56,525 B; jms 653 -> 580; bitwise libcalls
  93 -> 20 (all remaining have variable operands: printer `u32` masks, `1 << k`
  -> Layer B5). Full `run_tests.sh` green up to the size gate (expected FAIL).
- Takeaway for the next step: bitwise ops were not where the bytes are in
  this IR. Memory access (A4: DCL brackets, pointer-restore tails, 16-bit
  pointers) and the data model (Layer B) are.

## Continuation 2 — A4, Layer B data model, C1, asm core (in progress)

- A4: `-i4004-banked-ram` (off by default) drops all DCL brackets; killed-pointer
  loads skip the char restore; LDRAMi8 dst/address-pair overlap fixed via P0.
- C1: JMS/BBL by default (`-i4004-softcall` keeps the old convention); packer
  `_assert_call_depth` (<= 3 from main), `--gc-sections`, flat page-crossing
  relaxation fix-point; compile-time BranchRelax off by default; flat `crt0.s`
  (`crt0_banked.s` kept); `link_xl.ld` bank-0 map with `frames*` regions.
- A3: ADDi8ri/ADDi16ri; pre-RA `I4004ImmCompare` (BRCC*ri, sign tests, BRRAM
  fusion of a RAM read into its EQ/NE test); used-nibble compares; inline RAM
  intrinsics (`__ram_rd/wr/rsK/wsK`) incl. constant stores; per-callee call
  masks (`__leaf_*`/`__bus_*` preserve P4-P6).
- Layer B: Num = 4002 register handle (`src/ram.rs`, `src/bcd.rs`), flags in
  status chars, Row = print register, key FIFO in RAM, switches latched in RAM.
  Host 42/42 on all three oracles; `tests/bcd_equivalence.rs` (old vs new).
- asm core: `firmware/asm/num.s` (15 leaves), proven equal to the Rust twins
  on chips: `firmware/scripts/exec_num.py` 3000/3000 incl. register contracts.
- Backend exec suites: cmp 11180/11180, nibble 6288/6288, 20/20, 20/20.
- Size: .text 48,224 -> 9,456 B Rust (+~640 B asm); flat image still > 4096.
- Next: AssertZext on zeroext call results/args (LowerCallResult/
  LowerFormalArguments, I4004ISelLowering.cpp ~L462/L653); move div/sqrt/add/
  mul/round bookkeeping into asm entries; shrink the calculator layer (~5.4 KB)
  -- possibly printer/keyboard drivers into asm; then link, depth check, 42-run.
- Nothing committed.
