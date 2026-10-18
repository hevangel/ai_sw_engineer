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

### Multiplication, division, percent, EX, constants (M2) — to be specified from the listing
### Memory keys (M2)
### Square root (M2)

## Assumption ledger

| ID | Assumption | Why the docs don't settle it | How it is validated |
|---|---|---|---|
| A1 | An entry with more fractional digits than DP is truncated when aligned for `+`/`-`. | Manual says "set DP for the maximum entered decimal places" and is silent otherwise; no example exercises it. | Check against the annotated listing; add a recaptured transcript from the original ROM. |
| A2 | Accumulators keep the fractional precision they were built at; `=` rounds/truncates to the DP current at `=`. | Inferred from example 1-2 (DP changed before `=`). | 1-2 passes; listing review. |
| A3 | `C` prints a row with symbol `C` and no digits. | Manual lists `C` as the printed symbol; examples check only suffixes. | Recapture `C` on the original ROM. |
| A4 | In FL, `=` after `+`/`-` prints the total at DP like the fixed modes, truncating. | No FL addition example. | Recapture on the original ROM. |
