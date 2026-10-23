# Busicom 141-PF firmware — implementation plan

Goal: a Rust reimplementation of the 1971 calculator firmware that prints exactly what the original ROM prints for all 42 manual examples, compiled through LLVM to 4004 machine code and run as the ROM of the ai_hw_engineer board.

## Milestones

| | Scope | Exit criterion | Status |
|---|---|---|---|
| M0 | Title scaffold, oracle import, replay harness | `cargo test` runs all 42 examples and reports a score | ✅ done |
| M1 | BCD core; entry; `+ - ST = C CE SIGN`; DP alignment; rounding at total; overflow | examples 1-1…1-4, 14-1…14-3 pass | ✅ done |
| M2 | `* / % EX`, constants, chains, powers, mixed calc; memory keys; `M=+ M=-`; square root; FL precision | 42/42 on the host | ✅ done (42/42) |
| M3 | Machine drivers: keyboard scan via 4003, 8-key buffer, selectors, printer drum/hammer sequencing on TEST, lamps — tested against a board model on the host | same 42/42 through port-level simulation | ✅ done (42/42) |
| M4 | `llvm-i4004` backend sufficient for this crate (see `llvm-i4004/plans/`) | `build_image.sh` emits a 4001 ROM image | ✅ flat 4,079 B image (issue #18) |
| M5 | Image on ai_hw_engineer RTL board and `external/busicom` | 42/42 on the RTL; size vs 1280 / 4096 bytes reported | ✅ 2026-10-22: 42/42 on the RTL and on chips, 4,095 B (3.2× the 1,280 B original, 1 B under 4,096) |

## Fit plan (issue #18)

The 2026-10-19 → 10-21 sessions met every component milestone but the whole
firmware compiled to ~57 KB of `.text` and the 48-bank image printed 0/42. Issue
#18 ("Fable code review") replaces that path with three layers: A — backend
lowering (signed compares, native nibble ops, constants, memory access, bounds
checks); B — firmware data model (one digit per 4002 character, block
primitives in `regops.s`, `u8` only); C — `JMS`/`BBL` calls, nibble arguments,
an i4004 cost model. Progress is recorded per landing in
`report/size_history.md`; the size gate is `scripts/gen_worklist.py` (run by
`llvm-i4004/scripts/run_tests.sh`).

Rules:

- **No more banking machinery.** The bank latch, `__tobank` stubs, DCL-banked
  frames and bank-reload runners are frozen; no work goes into them.
- **Stop criteria.** If `.text` is above 25 KB after Layer A, or above 8 KB
  after Layer B, stop and re-profile; do not add paging.
- Budget: 4,096 B flat on the `xl` board, bank-0 RAM only; 1,280 B is tracked,
  not gating.

| # | Task | Status |
|---|---|---|
| 1 | P1 size gate + P2 scoreboard | ✅ 2026-10-22 |
| 2 | A2 signed compares (+ frame-slot offset fix found on the way) | ✅ 2026-10-22, `exec_cmp.py` 5,540/5,540 on chips |
| 3 | A1 native nibble patterns | ✅ 2026-10-22, `exec_nibble.py` 5,520/5,520 on chips; bitwise libcalls 93 → 20 (rest are variable-operand, B5) |
| 4 | A3/A4/A5 constants, memory access, bounds checks | ✅ A4 (with B1); A3/A5 not done as separate tasks, because the code they targeted moved into `asm/num.s` |
| 5–8 | B1–B5 firmware data model | ✅ 2026-10-22: one digit per 4002 character, digit loops and whole operations in `asm/num.s` (`exec_num.py` 4,600/4,600 on chips), drivers in `asm/drivers.s`, host 42/42 |
| 9 | C1 `JMS`/`BBL` convention | ✅ flat link, call graph 3 deep (checked by the packer), plus the MachineOutliner (JMS/JUN bodies) |
| 10–11 | C2 nibble args, C3 TTI | not done: the image fit without them |
| 12 | C4 intrinsics | ✅ for RAM access: `__ram_rd/wr/rs<k>/ws<k>` (`src/ram.rs`) lower inline to `SRC` + `RDM`/`WRM`/`RDk`/`WRk` (`I4004ISelLowering::LowerCall`); `digit`/`set_digit` go through them |
| 13 | 42-run on chips, then RTL | ✅ chips 42/42; RTL 42/42 (Verilator, xl board) after two drum-timing fixes; image 4,095 B |

## Design decisions

- **Digit-serial BCD, 16 nibbles per register** (`src/bcd.rs`), sign and decimal position kept beside the digits — the same representation the original keeps in 4002 registers. No binary integers wider than `u8` in the core, so the 4004 backend never needs 16/32-bit arithmetic for the calculator itself.
- Core separated from drivers so M1–M2 can be checked against the oracle on the host while the backend does not exist yet.
- Size is the dominant risk: 1280 bytes is hand-written-assembly density. The report will give size against 1280 bytes (original) and 4096 bytes (16 × 4001, the MCS-4 maximum); exceeding 1280 is expected at first and recorded, not hidden.
- The 3-level 4004 call stack means the backend will inline or use a RAM software stack; keep the core's call graph shallow (no recursion, few levels).
