# 2026-10-19 (cont.) — Tier-A regops primitives; far-branch relaxation is the gate

Branch `system/busicom_141pf-on-4004`. No PR. Every tool call timeout-wrapped;
host tests 42/42 exact at every commit.

## Done this run (committed + pushed)
- **regops block primitives** (Tier-A firmware/runtime co-design). The packed
  `[u8;8]` digit layout aligns so digit i is char i of the array's 16-char RAM
  region, so a whole-register BCD loop is one SRC + RDM/WRM/INC loop instead of
  16 inlined Rust mask/shift iterations.
  - `firmware/src/regops.rs`: `extern "C"` reg_add/reg_sub/reg_cmp/reg_is_zero/
    reg_copy/reg_clear, host Rust impl byte-identical to the loops they replace.
  - `bcd.rs`: add_mag/sub_mag/cmp_mag/is_zero call regops.
  - `llvm-i4004/rt/regops.s`: target asm for all six (tight 16-digit loops).
    **Execution-tested on external/chips vs a host oracle: 10/10 pass**
    (carry/borrow chains, all three cmp orderings) via new
    rt/tests/run_regops_tests.sh + gen_regops_test.py + regops_oracle.py.
  - build_image.sh links regops.s.
  - firmware .text 41,424 -> 40,736 B.

## CRITICAL GATE reached: far-branch relaxation
build_image.sh now links past symbols and fails on the PAGE-LOCAL constraint:
functions exceed 256 B (e.g. `memory_add` 760 B) and the 4004's JCN/ISZ 8-bit
address field only reaches within the current 256-byte ROM page. **No image
links until conditional branches are made page-independent**, independent of
total size. Recommended fix (option a, in the backend): emit every conditional
branch as `JCN !cc, .skip ; JUN Dest ; .skip:` (JCN skips a 3-byte JUN — same
page; JUN reaches the full 12-bit space), +2 B/branch (~716), then drop
`--pad-pages`. Alternative (b): fixed-point relaxation in i4004_pack.py. This is
the next unit of work and is itself multi-hour. Details in report Status.

## Still ahead after the gate
Total size still ~10x over: convert the heavy mul/div/sqrt WIDE=34 scratch loops
to register-backed regops (Tier B), MachineOutliner via the SOFTWARE stack
(JMS would overflow the 3-level hardware stack), then the 42-example chips run.

## Validation
regops chips 10/10; host `cargo test` 42/42 exact; firmware compiles to object
(43/43); lit 26/26 (unchanged). Size 40,736 B (from 45,714).
