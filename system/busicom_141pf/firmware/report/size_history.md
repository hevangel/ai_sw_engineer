# Busicom 141-PF firmware — size scoreboard

One row per landing that touches the firmware or the `llvm-i4004` backend
(AGENTS.md, "Size scoreboard"). The plan being executed is issue #18 ("Fable
code review": Layers A–C); the budget is the 4,096-byte 16 × 4001 ceiling on the
`xl` board, with the 1,280-byte original ROM tracked as a stretch.

How each column is measured (inside the `ai-sw-engineer` container):

- **.text** — `llc -mtriple=i4004 -function-sections -filetype=obj
  build/firmware_i4004.ll`, sum of every `.text*` section (`llvm-objdump -h`).
  `gen_worklist.py` prints the same number as its size gate.
- **image** — `build_image.sh` with `link_xl.ld`, flat (no `BANKS`); "—" while
  the firmware cannot link flat under 4,096 B.
- **largest fn** — biggest `.text.<fn>` section.
- **jms / bitwise libcalls** — counts in `llc` asm output: all `jms`, and `jms
  __{and,or,xor,lshr,ashl,ashr}{qi,hi}3`.
- **host** — `cargo test` (12 unit + manual_examples + port_level +
  port_trace = 42 manual rows). **target** — `run_image.sh` + `compare.sh`
  rows matched out of 42.

| Date | Commit | Change | .text B | image B | largest fn B | jms | bitwise libcalls | host | target |
|---|---|---|---|---|---|---|---|---|---|
| 2026-10-22 | fcc21e4 (main) | baseline as merged; IR rebuilt by `build_target_ir.sh` | 56,945 | — | 1,790 (`mul_div_key`) | 653 | 93 | 42/42 | 0/42 (48-bank image, 2026-10-21) |
| 2026-10-22 | `llvm/i4004-fit` | A2 signed compares; frame-slot offset fix; P1 size gate | 57,010 | — | 1,790 (`mul_div_key`) | 653 | 93 | 42/42 | not run |
| 2026-10-22 | `llvm/i4004-fit` | A1 native nibble ops (LOGICI/SHIFTI/NIBSEL) | 56,525 | — | 1,791 (`mul_div_key`) | 580 | 20 | 42/42 | not run |

Notes:

- The 56,945 B baseline is larger than the 42,114 B quoted in
  `llvm-i4004/report/final_report.md`; that figure was taken on an earlier IR
  (before the `#[inline(never)]` bank-window splits merged in #16/#17). This
  table re-measures from the IR the current sources produce.
- +65 B for A2 is the price of correct signed relations (13 B per signed
  compare); the frame-offset fix changes no instruction count.
- A1 removed 73 of 93 bitwise libcalls (−78%; the plan's exit criterion is
  −90%) but only 485 B (−0.9%). The remaining 20 all have variable operands
  (`x | y` of unknown masks, `1 << k`, `0x8b >> k`), mostly the printer's `u32`
  sector masks. Inline variable bitwise ops would need per-bit branches and be
  bigger than the call, so they belong to B5 (printer data model). The small
  byte saving shows the review's point: this IR has few nibble idioms left,
  because the firmware's packed-BCD accessors were already inlined into
  load/store-heavy code. The size is in memory traffic (RC-1, A4) and the data
  model (Layer B), not in the bitwise ops.

### 2026-10-22 continuation: asm entries, flat call graph, outliner

Same measurement (`.text` = compiled Rust only, from `gen_worklist.py`; image =
the flat `i4004_pack` link of Rust + `asm/num.s` + `asm/drivers.s` + runtime,
padding and page relaxation included; uncommitted working tree on
`llvm/i4004-fit`, base fcc21e4).

| Date | Commit | Change | .text B | image B | largest fn B | jms | bitwise libcalls | host | target |
|---|---|---|---|---|---|---|---|---|---|
| 2026-10-22 | wt | A4 memory access + B1 RAM data model (Num handles, RAM flags) | 48,224 → 22,369 | — | — | — | — | 42/42 | not run |
| 2026-10-22 | wt | B2 digit loops in `asm/num.s` leaves; `__n_div` entry | 8,156 | 9,727 | 1,888 (`run`) | — | — | 42/42 | not run |
| 2026-10-22 | wt | `__n_mul/add/round/sqrt` entries, `__leaf_n_width` | 6,304 | 8,585 | 1,888 (`run`) | — | — | 42/42 | not run |
| 2026-10-22 | wt | selectors/lamps latched in RAM; zero compares `LD; JCN`; `__n_row` | 5,220 | — | 1,511 (`run`) | — | — | 42/42 | not run |
| 2026-10-22 | wt | i8 setcc result type; packed `__leaf_n_cp` | 4,747 | — | 1,447 (`run`) | — | — | 42/42 | not run |
| 2026-10-22 | wt | drivers in `asm/drivers.s`; call graph flattened to depth 3 | 4,508 | 7,243 | 755 (`main`) | — | — | 42/42 | not run |
| 2026-10-22 | wt | `Printer::number` → one `__pr_row` call; shared mul/div step | 3,537 | — | 755 (`main`) | — | — | 42/42 | not run |
| 2026-10-22 | wt | nibble dispatch, FIFO layout, `reset()`, `take_entry` | 3,319 | 5,845 | 526 (`main`) | — | — | 42/42 | not run |
| 2026-10-22 | wt | MachineOutliner (JMS/JUN bodies); disjoint `or` → `add`; div fix | 2,964 | 5,429 | 494 (`main`) | 289 | 0 | 42/42 | not run |

| 2026-10-22 | wt | asm rewrites (`__n_div` 368 B, `__n_sqrt`, `__n_compute`, `__pr_row`), page-filling packer order, `i8` bool return | 2,076 | 4,079 | 342 (`main`) | 201 | 0 | 42/42 | chips 42/42 |
| 2026-10-22 | wt | `__pr_print` counts TEST edges (phase-independent drum sync), waits an edge before the paper advance | 2,076 | 4,095 | 342 (`main`) | 201 | 0 | 42/42 | chips 42/42, RTL 42/42 |

- The image fits: 4,079 B of 4,096 at the first of the last two rows, and 4,095
  B (1 B free) after the drum fixes (+12 B in `asm/drivers.s`, +4 B of page relaxation in `asm/num.s`). The 4,079 B map has no gaps; it holds
  2,096 B of compiled Rust (2,076 B `.text` + 20 B of page relaxation),
  1,626 B of `asm/num.s`, 355 B of `asm/drivers.s` and 2 B of reset vector. On the way from 5,429 B the image was 5,307, 4,954,
  4,119 and 4,088 B; the other columns were not measured for those builds.
- The row before the last one was over the 4,096 B budget by 1,333 B. Of the 5,429 B, about 2.3 KB
  is hand-written asm (`num.s` ~1.96 KB, drivers ~0.4 KB) and ~150 B is page
  padding/relaxation.
- The call-depth fix (3-level stack) cost inlining but was required: the old
  graph was 7 deep and could not have run.
