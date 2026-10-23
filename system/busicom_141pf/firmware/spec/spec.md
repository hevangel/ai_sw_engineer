# Busicom 141-PF firmware — specification

Behavior of the original 1971 firmware as seen at the keyboard, switches, printer and lamps. Primary source: the Unicom 141 operating manual (summarized in `$AI_HW_ENGINEER/system/busicom_141pf/spec/reference/Unicom_141P_manual.md`); where the manual and the original ROM disagree, the ROM wins (see `../oracle/README.md`). Algorithms and register layout are taken from the annotated disassembly (cited in `../original/provenance.md`).

## Inputs

- 30 keys: `0`–`9`, `00`, `.`, `SIGN`, `+`, `-`, `*`, `/`, `=`, `%`, `ST` (◇), `EX`, `CE`, `C`, `CM`, `RM`, `M+`, `M-`, `M=+`, `M=-`, `SQRT`.
- Decimal selector DP ∈ {0,1,2,3,4,5,6,8}; rounding selector FL / 5/4 / IN.
- The 8-key input buffer is behavior of the keyboard driver (M3), not of the arithmetic core.

## State

Five working registers — entry, subtotal, main total, two multiply/divide registers — and one memory. Each holds up to 14 digits, a sign, and a decimal position.

**Representation (since issue #18).** As in the original ROM, each register is
one 4002 RAM register:
- **Digits:** 16 main characters, one BCD digit each.
- **Sign and decimal position:** status characters.
- **Calculator flags** (entering, point, mode, pending operation, lamps): status
  S3 of the registers.

Rust names a register by a 1-byte `Num` handle (`src/bcd.rs`); `src/ram.rs`
reads and writes characters. The bank-0 layout is in
`../platform/memory_map.md`.

## Outputs

- One printed row per print event: 15 numeric columns (14 digits + decimal point), symbol columns A and B, red ink, rounding mark (`^` in column A).
- Lamps: memory, overflow, negative.

## Behavior (by milestone)

### Number entry
Up to 14 digits; `.` once; `00` enters two zeros; `SIGN` makes the entry negative. Entry digits print only when a function key prints them.

### Addition / subtraction (M1)
- `+`/`-` after an entry: the entry is aligned to DP fractional digits, printed with `+`/`-`, and added to / subtracted from both subtotal and main total. With no new entry, the previous entry is repeated.
- Negative values print red.
- `ST` after `+`/`-`: print subtotal with `ST`, clear subtotal. `ST` right after number entry: print the entry exactly as typed with `#` (reference/date) and change nothing else.
- `=` after `+`/`-`: print main total with `*` at DP, apply the rounding switch when the total has more fractional digits than DP (rounded-up rows carry `^`), clear both accumulators.
- Capacity: an aligned entry or accumulator result above 14 digits prints the dotted overflow line, lights overflow, and leaves the accumulators unchanged. `CE` clears the entry (to an explicit zero) and the overflow lamp.

### Multiplication, division, percent, EX, constants (M2)

Modelled on the annotated disassembly's operation table (§4.4), which keys the
behaviour off three state fields: `MODE` (`RR.S2`: new number / last-was-mul-div
/ last-was-add-sub), `MOP` (`DR.S2`: none / multiply / divide / constant-multiply
/ constant-divide) and the digit-entry flag. Registers follow the listing's
names: working `WR`, dividend-multiplicand `DR`, constant `CR`, result `RR`,
subtotal `SR`, main total `TR`, memory `MR`.

- **`*` / `/`** latch the first operand into `DR` and `CR`, set `MOP`, and print
  the operand with the `*op` / `/` symbol. A `*`/`/` pressed while a multiply or
  divide is already pending **chains**: the pending product/quotient is computed
  at floating precision and becomes the next first operand. A `*`/`/` right after
  `+`/`-` reads and clears the main total and starts the operation on it (mixed
  calculation). Pressing the other of `*`/`/` with no new entry corrects the
  operation.
- **`=`** prints the second operand with `=`, computes `DR (op) WR`, prints the
  result with the total `*`, and leaves the result in `WR`. The operand that was
  held (multiplicand after `*`, divisor after `/`) is retained in `CR` as the
  **constant**, so a further `operand =` reuses it (constant calculation). A bare
  repeated `=` raises powers (the previous result is multiplied by the constant
  again).
- **Result precision.** Multiplication is exact: the product keeps the sum of the
  operands' fractional digits (floating). Division in **FL** keeps 13 significant
  digits, truncated (M2-D1). In the fixed modes the result is rounded (5/4) or
  truncated (IN) to the decimal selector; a rounded-up result carries `^`.
  Intermediate chain products/quotients always use floating precision. The
  **displayed, rounded** value is what is left in `WR`, so a following `+`/`-`
  aligns to the printed number, not the raw high-precision intermediate.
- **`%`** performs the pending operation and scales for percentage: percentage
  multiplication treats the operand as `n/100` (multiply by `0.0n`); percentage
  division scales the quotient by 100. The result prints with the total `*`.
- **`EX`** exchanges the two multiply/divide operands (`WR` ⇄ `DR`), tracking the
  new first operand in `CR`; it prints the current entry with `EX`. This lets a
  constant be placed on either side (e.g. a constant multiplier via `*`+`EX`).
- **Capacity.** A floating product/quotient may not exceed 14 integer digits; a
  fixed-mode result may not exceed `14` printed digits (`14 − DP` integer digits).
  Overflow prints the dotted line and lights the overflow lamp.

### Memory keys (M2)

One memory register `MR`, with the memory lamp lit whenever `MR ≠ 0`.

- **`M+` / `M-`** add/subtract the current entry (or last operand) to/from `MR`;
  they print nothing on their own.
- **`M=+` / `M=-`** behave like `=` — print the second operand with `=`, compute
  the pending product/quotient — then add/subtract that result to/from `MR`. The
  **printed row is the product/quotient** (with the `M+` / `M-` symbol), rounded
  to DP; `MR` accumulates that same rounded value. `CM` later shows the running
  sum.
- **`RM`** recalls `MR` into `WR` and prints it with `RM` (rescaled to DP, no `^`).
- **`CM`** prints `MR` with `CM` (rescaled to DP, no `^`), copies it to `WR`, then
  clears `MR` and the memory lamp.

### Square root (M2)

Available only on the recovered five-ROM image. `SQRT` prints the radicand with
the square-root symbol, then the root with the square-root symbol. The root is
computed digit-by-digit by the odd-number-subtraction method of the listing
(§4.5), `sqrt(|WR|)` (sign ignored). In **FL** the root fills to 14 significant
digits (truncated); in the fixed modes it is rounded/truncated to DP. The
square-root result row never carries the `^` mark. The radicand prints at its
entered scale when freshly typed, otherwise rescaled to DP.

## Assumption ledger

| ID | Assumption | Why the docs don't settle it | How it is validated |
|---|---|---|---|
| A1 | An entry with more fractional digits than DP is truncated when aligned for `+`/`-`. | Manual says "set DP for the maximum entered decimal places" and is silent otherwise; no example exercises it. | Check against the annotated listing; add a recaptured transcript from the original ROM. |
| A2 | Accumulators keep the fractional precision they were built at; `=` rounds/truncates to the DP current at `=`. | Inferred from example 1-2 (DP changed before `=`). | 1-2 passes; listing review. |
| A3 | `C` prints a row with symbol `C` and the cleared value `0` (not a blank row). | Manual lists `C` as the printed symbol; the JSON examples check only step suffixes and never include a `C` row. | **Corrected against `oracle/rom_port_trace.txt`** (`tests/port_trace.rs`): the original ROM prints `0 C`, not the blank row A3 originally guessed. Fixed in `Calculator::clear`; see `failure_notes/2026-10-18-clear-prints-zero.md`. |
| A4 | In FL, `=` after `+`/`-` prints the total at DP like the fixed modes, truncating. | No FL addition example. | Recapture on the original ROM. |
| M2-D1 | FL division keeps **13 significant digits**, truncated (integer part always kept whole); FL multiplication is exact at the summed scale. | The manual says FL "uses the arithmetic decimal position" but gives no digit budget; the ROM's register layout decides it. | Matches the recovered ROM for 3-1 (`6.666666666666`), 3-4 (`25.98225602027`), 4-2 (`66.66666666666`). Confirm against the listing's division loop. |
| M2-D2 | A mul/div/`%` result leaves the **rounded, displayed** value in `WR`, so a following `+`/`-`/chain aligns to the printed number, not the raw intermediate. | Not stated; needed to reproduce 6-1's `57.68 +` → `100.00 *`. | 6-1 passes. Confirm against the listing's `MOV WR,RR` + print path. |
| M2-S1 | FL square root fills to **14 significant digits** (truncated); fixed modes round/truncate to DP. The result row never carries `^`. | The manual gives only worked examples; no stated width. | Matches 11-1 (`12345.750000000`), 11-2 (`1.414214`), 11-3 (`14.422205`). Confirm against the §4.5 SQRT pseudo-code. |
| M2-O1 | The `=`/`M=±` second-operand row prints at the entered scale when the operation began from a freshly typed entry, else rescaled to DP. | The ROM's digit-entry (`MENT`) state decides this; the manual is silent. | Reproduces 11-2 (`2.000000 =`, repeat chain) vs 11-3 (`8 =`, entry-fed). Confirm against the listing's digit-point handling. |
| M3-H1 | The port-level HAL is four port ops + TEST (`rom_port_write/read`, `ram_port_write`, `test_pin`) with the chip-select and bit assignment in `src/hal.rs`. | The 4004 does I/O via `SRC`+`WRR`/`RDR`/`WMP` and `JCN`-on-`TEST`; the exact chip/bit map is the board's, not the ROM's. | Matches ai_hw_engineer board spec §3–4 + `busicom_141pf.sv`; the original ROM on `external/chips` through the same wiring scores 42/42 (`oracle/port_level_validation.md`). |
| M3-K1 | Keyboard scan walks a single selection bit through 4003 #0; the selected row is the one set bit; rows Q8/Q9 return the selectors. The 8-key buffer is a FIFO that drops the 9th key. | The real machine scans 40×/s and buffers 8 words (manual §12); the RTL virtual platform drops the buffer entirely (its §5.1 deviation). | Keyboard round-trip asserted key+switch recovery for every one of the 42 examples; buffer depth has its own unit test. |
| M3-P1 | The host board model advances one drum sector per `test_pin()` sample (the driver samples TEST once per sector), rather than modelling the two-half-spin-per-sector analog timing. | Cycle-accurate analog hammer timing is a non-goal (board spec §5.1, §6); only the sector under the hammers at a hammer edge affects the printed row. | The reconstructed `{value,symbol,red,rounded}` row is identical to the original-ROM trace, which uses the real half-spin pacing; port-level replay 42/42. |
| A6 | On `×`/`÷` the ROM prints the first-operand row with the op glyph for a **recalled or accumulated** operand too (not only a freshly typed one), and on a chain it echoes the new operand before computing the pending product; pressing a second op key with no new entry is an **operation correction** that reprints only the op glyph with a **blank** value. A freshly typed operand echoes at its entered scale; a recalled one is aligned to DP. | `oracle/rom_port_trace.txt` records the ROM's full tape; the JSON oracle checks only step suffixes and never listed the recalled-operand echo (5-1 `12.4 ÷`, 9-1 `68.94 ×`, 8-1 `1368 ×`) or the blank op-correction rows (2-8 `× ÷ ×`). | **Reproduced.** `Calculator::mul_div_key` prints the echo in every branch (recalled, chain, mixed-total) with `operand_shown` for the fresh-vs-DP scale, and a blank-value row for an op correction; `tests/port_trace.rs` asserts the full row stream exactly, 42/42. |
| A7 | A bare `M+` / `M-` key prints `value M+` / `value M-` in symbol column A, the operand aligned to DP exactly like a `+`/`-` operand (10-1 `10.00 M+` from typed `10` at DP 2; 11-2 `2.000000 M+` at DP 6). | `oracle/rom_port_trace.txt` (9-1 `6.89 M-`, 10-1 `10.00 M+`, 11-2 `2.000000 M+`); the JSON oracle only checks per-step suffixes, which never include the echo. | **Reproduced.** `Calculator::memory_add` prints the DP-aligned echo in column A; asserted exactly by `tests/port_trace.rs`. |
| A8 | The drum fires symbol columns A and B independently; some rows carry two glyphs (`C`+`M` for CM, `R`+`M` for RM, `^`+result for a rounded result; an `M=±` result sits in column B while a bare `M+`/`M-` key echo sits in column A). | ai_hw_engineer board spec §4.2 (hammer-word bit 0 = column A, bit 1 = column B); `oracle/rom_port_trace.txt`. The sanctioned JSON oracle collapses both columns into a single `symbol` token, which is why it never surfaced. | **Reproduced.** `print::Row` carries `sym_a: SymA` + `sym_b: SymB` (the rounded `^` is `SymA::Rounded`); the core sets both columns, the printer driver fires each at its own sector, and `tests/port_trace.rs` asserts each column's glyph exactly. The JSON-oracle tests fold the two columns back to the single token (`Row::oracle_symbol`). |
| A9 | The minus-sign lamp (RAM1 bit 2) tracks the sign of **WR at idle** (so it lights on a negative total/result but not on a `+`/`-` operand that merely leaves the accumulator negative), and the ROM also drives RAM1 **bit 3** — the second bit of the WR sign-status character `WR.S0.bit1` — which the trace shows set on overflow (nibble 10) and on a negative total/result (nibble 12). | **Sourced** to the annotated listing's status-light routine (`010 jms $173 … 01d wmp`): the RAM1 nibble is `8·WR.S0.bit1 + 4·WR.S0.bit0 + 2·overflow + (MR≠0)`, so bit 2 = `WR.S0.bit0` (WR sign) and bit 3 = `WR.S0.bit1`. The listing's port map comments bit 3 "not used", but the routine writes it. | **Reproduced.** `Calculator::press` sets the negative lamp from `WR.neg`; `driver_lamps::lamp_nibble` emits bit 3. ASSUMPTION: `WR.S0.bit1`'s full semantics are not recovered from the disassembly, but across all 42 examples it is set exactly when the row is an overflow or a negative total/result, so we reproduce it as `overflow \|\| negative`. `tests/port_trace.rs` asserts the full 4-bit nibble, 42/42. |
| M2-D3 | A quotient too small for FL's 13 significant places prints 0 (the M2 code reported overflow when its fixed buffer ran out). | No example divides to below 10^-13; the manual does not say. | `tests/bcd_equivalence.rs` (`div_truncates_exactly`) checks the rule against exact decimal arithmetic; `exec_num.py` checks `__n_div` against its twin. |
| M3-K2 | One key event per press-and-release: a key is accepted on the first scan pass that sees it down after a pass that saw no key, so holding a key never repeats it. The selectors are latched into RAM (`SW_DP`, `SW_ROUND`) on every pass. | The manual describes the 8-key buffer, not key repeat; the simulators hold each key for 64 drum half-spins. | Host `scan_latch` and target `__kb_scan` (`asm/drivers.s`); 42/42 on chips and the RTL. |
| M3-K3 | The target scans the keyboard only between keys (`main.rs` run loop), so it never holds more than one key and does not buffer keys typed during a calculation or a print, unlike the original's 8-key type-ahead. The host `KeyBuffer` keeps the 8-deep FIFO. | Implementation choice for ROM size; type-ahead is not exercised by the 42 examples (each key is held longer than one print). | Known gap; not covered by any oracle. |
| M3-P2 | Drum timing on the target: TEST changes every half-spin, a sector lasts two half-spins, and the index (ROM2 bit 0) rises one half-spin before sector 0. `__pr_print` counts TEST *edges*, never the level: sector 0 starts at the first edge after the index is seen, each later sector two edges on. | The board spec gives the half-spin toggle but not which TEST level a `JCN` sees; external/chips and the RTL present it in opposite phase (`failure_notes/2026-10-22-busicom_141pf-drum-phase.md`). | 42/42 on both the RTL and chips. |
| M3-P3 | After the last sector's hammers, the printer waits one more TEST edge before the paper-advance pulse. | Hammer and advance timing is analog on the real machine; the RTL board drops a hammer that shares a poll window with the advance. | 42/42 on the RTL (symbols `M`/`R` at sector 12). |
| M5-W1 | A printed number has width ≤ 14 and scale ≤ 13, so it fits the 15 columns; `__pr_row` takes both mod 16. | Holds because every printed value is rounded to DP or to FL's 13 digits first; not a documented property. | `print.rs` `Row::number` (twin) and the 42-example runs. |
