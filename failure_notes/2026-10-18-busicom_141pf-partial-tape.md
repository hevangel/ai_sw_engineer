# The reimplementation printed a thinner tape than the ROM (A6–A9)

- Area: system/busicom_141pf/firmware
- Found by: the original 1280-byte ROM's own port trace (`oracle/rom_port_trace.txt`), via `tests/port_trace.rs`

## What broke

Compared against the original ROM's full printed tape, the Rust core omitted or
mis-shaped several classes of printed row. The ROM prints a richer tape than a
single-glyph-per-row model can express:

- **A6 — operand echo on `×`/`÷`.** The ROM reprints the first operand with the
  op glyph not only for a freshly typed entry but also for a recalled or
  accumulated one (9-1 `68.94 ×` after `RM`, 3-6 `123.45 ÷` after `M+`, 8-1
  `1368 ×` from a recalled total), echoes the operand on a chain before
  computing (2-4 `4.56 ×`, 3-4 `6 ÷`, 5-1 `12.4 ÷`), and on an operation
  correction reprints only the op glyph with a **blank** value (2-8 `× ÷ ×`).
  The core printed an operand row only for a freshly typed entry.
- **A7 — bare `M+`/`M-` key echo.** The ROM prints `value M+` / `value M-`
  (operand aligned to DP) in symbol column A (9-1 `6.89 M-`, 10-1 `10.00 M+`,
  11-2 `2.000000 M+`). The core's `memory_add` printed nothing.
- **A8 — two symbol columns.** The Model-102 drum fires symbol columns A and B
  independently; some rows carry two glyphs — `C`+`M` for CM, `R`+`M` for RM, a
  rounded `^` in column A beside the result symbol in B, an `M=±` result in
  column B while a bare `M+`/`M-` key echo is in column A. The core's single
  `Symbol` carried one glyph per row.
- **A9 — lamp nibble.** The minus-sign lamp (RAM1 bit 2) tracks the sign of WR
  at idle, so it lights on a negative total/result but not on a `+`/`-` operand
  that merely leaves the accumulator negative; and the ROM also drives RAM1
  **bit 3** (`WR.S0.bit1`), set on overflow (nibble 10) and on a negative total
  (nibble 12). The core updated `negative` from the accumulator on every
  `+`/`-` and never set bit 3.

## Root cause — and why our tests missed it

The bug was in the **Rust reimplementation** (the arithmetic core and its row
model), not the drivers, backend, or board model. The board model faithfully
rendered whatever row the core produced.

Two things hid it. First, the core's `print::Row` carried a single `Symbol`, a
structural limit that *could not* represent the ROM's two independent symbol
columns — so A8 was invisible by construction. Second, the title's sanctioned
oracle `oracle/manual_examples.json` checks each step's tape as a **suffix** and
collapses both drum columns into one `symbol` token; the echo rows (A6/A7) are
not at a step's suffix and the second column (A8) is folded away, so neither the
`manual_examples.rs` nor the `port_level.rs` replay ever constrained them. The
negative lamp and bit 3 (A9) had no JSON checkpoint at all. When
`tests/port_trace.rs` was first added it documented these as gated ASSUMPTIONs
(A6–A9) and compared our rows only as an in-order *subsequence* with a
glyph-*subset* match and a memory/overflow-only lamp mask — an honest but
deliberately loose comparison that let the thinner tape pass.

## Fix and re-verification

- `print::Row` now carries two symbol columns, `sym_a: SymA` and `sym_b: SymB`
  (the rounded `^` mark is `SymA::Rounded`, column A sector 7); `driver_printer`
  maps each column's glyph enum to its own drum sector and fires both.
- `Calculator::mul_div_key` prints the operand echo in the recalled, chain and
  mixed-total branches (with `operand_shown` for the ROM's fresh-vs-DP operand
  scale) and a blank-value row for an operation correction (A6).
- `Calculator::memory_add` prints the DP-aligned `M+`/`M-` key echo in column A
  (A7).
- `recall_memory`/`clear_memory` print `R`+`M` / `C`+`M` across both columns; a
  zero register prints the bare `0` the ROM shows, not a DP-padded zero (A8 +
  zero formatting).
- The minus-sign lamp is set from `WR.neg` at the end of every `press`, and
  `driver_lamps::lamp_nibble` emits bit 3 (A9). Both are sourced to the
  annotated listing's status-light routine, whose nibble is
  `8·WR.S0.bit1 + 4·WR.S0.bit0 + 2·overflow + (MR≠0)`.
- `tests/port_trace.rs` was rewritten to an **exact** comparison: the full
  ordered row stream (value string, column-A glyph, column-B glyph, red) and the
  full 4-bit lamp nibble per step must equal the trace, with no subsequence /
  subset / zero / lamp-mask relaxations.

Re-verified in the Docker toolchain container (`scripts/docker_run.sh` →
`scripts/run_all.sh`): `cargo fmt --check` and `cargo clippy -D warnings` clean;
`manual_examples.rs`, `port_level.rs` and the exact `port_trace.rs` all 42/42;
12/12 unit tests. Mutation checks confirm the exact test bites: removing the A7
echo fails 4 examples on row count, swapping the two symbol columns fails nearly
all, clearing bit 3 fails every overflow and negative-total row.

## Prevention

- The structural cause (one glyph per row) is gone: the row model now has the
  same two symbol columns as the hardware, so an A8-class gap cannot recur
  silently.
- `tests/port_trace.rs` compares the ROM's **entire** tape exactly (every row,
  both columns, the full lamp nibble), not a suffix or a subsequence, so a
  future thinning of the printed tape fails immediately.
- Spec `A6`–`A9` were updated from "gated / not fixed" to "reproduced", each
  citing its source (the annotated listing for the lamp nibble, the board spec
  §4.2 for the two columns, the port trace for the echo rows). The one residual
  unknown — the full meaning of `WR.S0.bit1` — is recorded as an explicit
  ASSUMPTION in `A9`, `driver_lamps.rs` and `hal::RAM1_STATUS`, with the
  observable bit still reproduced.
