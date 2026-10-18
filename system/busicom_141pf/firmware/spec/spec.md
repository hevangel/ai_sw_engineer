# Busicom 141-PF firmware — specification

Behavior of the original 1971 firmware as seen at the keyboard, switches, printer and lamps. Primary source: the Unicom 141 operating manual (summarized in `$AI_HW_ENGINEER/system/busicom_141pf/spec/reference/Unicom_141P_manual.md`); where the manual and the original ROM disagree, the ROM wins (see `../oracle/README.md`). Algorithms and register layout are taken from the annotated disassembly (cited in `../original/provenance.md`).

## Inputs

- 30 keys: `0`–`9`, `00`, `.`, `SIGN`, `+`, `-`, `*`, `/`, `=`, `%`, `ST` (◇), `EX`, `CE`, `C`, `CM`, `RM`, `M+`, `M-`, `M=+`, `M=-`, `SQRT`.
- Decimal selector DP ∈ {0,1,2,3,4,5,6,8}; rounding selector FL / 5/4 / IN.
- The 8-key input buffer is behavior of the keyboard driver (M3), not of the arithmetic core.

## State

Five working registers — entry, subtotal, main total, two multiply/divide registers — and one memory. Each holds up to 14 digits, a sign, and a decimal position.

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
| A3 | `C` prints a row with symbol `C` and no digits. | Manual lists `C` as the printed symbol; examples check only suffixes. | Recapture `C` on the original ROM. |
| A4 | In FL, `=` after `+`/`-` prints the total at DP like the fixed modes, truncating. | No FL addition example. | Recapture on the original ROM. |
| M2-D1 | FL division keeps **13 significant digits**, truncated (integer part always kept whole); FL multiplication is exact at the summed scale. | The manual says FL "uses the arithmetic decimal position" but gives no digit budget; the ROM's register layout decides it. | Matches the recovered ROM for 3-1 (`6.666666666666`), 3-4 (`25.98225602027`), 4-2 (`66.66666666666`). Confirm against the listing's division loop. |
| M2-D2 | A mul/div/`%` result leaves the **rounded, displayed** value in `WR`, so a following `+`/`-`/chain aligns to the printed number, not the raw intermediate. | Not stated; needed to reproduce 6-1's `57.68 +` → `100.00 *`. | 6-1 passes. Confirm against the listing's `MOV WR,RR` + print path. |
| M2-S1 | FL square root fills to **14 significant digits** (truncated); fixed modes round/truncate to DP. The result row never carries `^`. | The manual gives only worked examples; no stated width. | Matches 11-1 (`12345.750000000`), 11-2 (`1.414214`), 11-3 (`14.422205`). Confirm against the §4.5 SQRT pseudo-code. |
| M2-O1 | The `=`/`M=±` second-operand row prints at the entered scale when the operation began from a freshly typed entry, else rescaled to DP. | The ROM's digit-entry (`MENT`) state decides this; the manual is silent. | Reproduces 11-2 (`2.000000 =`, repeat chain) vs 11-3 (`8 =`, entry-fed). Confirm against the listing's digit-point handling. |
| M3-H1 | The port-level HAL is four port ops + TEST (`rom_port_write/read`, `ram_port_write`, `test_pin`) with the chip-select and bit assignment in `src/hal.rs`. | The 4004 does I/O via `SRC`+`WRR`/`RDR`/`WMP` and `JCN`-on-`TEST`; the exact chip/bit map is the board's, not the ROM's. | Matches ai_hw_engineer board spec §3–4 + `busicom_141pf.sv`; the original ROM on `external/chips` through the same wiring scores 42/42 (`oracle/port_level_validation.md`). |
| M3-K1 | Keyboard scan walks a single selection bit through 4003 #0; the selected row is the one set bit; rows Q8/Q9 return the selectors. The 8-key buffer is a FIFO that drops the 9th key. | The real machine scans 40×/s and buffers 8 words (manual §12); the RTL virtual platform drops the buffer entirely (its §5.1 deviation). | Keyboard round-trip asserted key+switch recovery for every one of the 42 examples; buffer depth has its own unit test. |
| M3-P1 | The host board model advances one drum sector per `test_pin()` sample (the driver samples TEST once per sector), rather than modelling the two-half-spin-per-sector analog timing. | Cycle-accurate analog hammer timing is a non-goal (board spec §5.1, §6); only the sector under the hammers at a hammer edge affects the printed row. | The reconstructed `{value,symbol,red,rounded}` row is identical to the original-ROM trace, which uses the real half-spin pacing; port-level replay 42/42. |
