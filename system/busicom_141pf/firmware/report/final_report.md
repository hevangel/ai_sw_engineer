# Busicom 141-PF firmware — report

Status at milestone M2 (2026-10-18).

| Check | Result |
|---|---|
| Host replay vs original ROM (recovered-ROM profile) | **42/42** examples, all 135 checkpoints |
| BCD unit tests | 9/9 |
| `cargo fmt --check`, `clippy -D warnings` | clean |
| `scripts/run_all.sh` (fmt, clippy, test, host build + IR) | clean (target/sim stages skipped — `llvm-i4004` is M4) |
| Target image size vs 1280-byte ROM | n/a — `llvm-i4004` backend not started |
| Primary simulator (ai_hw_engineer RTL) | n/a — needs the backend (M4/M5) |
| Secondary emulator (`external/busicom`) | n/a |

Toolchain: `ai-sw-engineer:latest` Docker image.

## What M2 added

- `src/bcd.rs`: exact BCD multiplication (schoolbook into a wide `u8` buffer),
  long division with a floating 13-significant-digit result and a fixed-DP
  result, digit-by-digit square root (odd-subtraction method), and `round_at`
  for the rounding switch. All digit values stay `u8`; no `u16`+ arithmetic and
  no recursion.
- `src/lib.rs`: the full M2 state machine — `* / % EX`, constants (K mode),
  chains, powers, mixed calculation, memory keys (`CM RM M+ M- M=+ M=-`) and
  square root — modelled on the annotated disassembly's operation table (§4.4),
  keyed off `MODE`/`MOP`/digit-entry state over the `WR/DR/CR/RR/SR/TR/MR`
  registers.
- `scripts/run_all.sh`: resolve the scripts directory absolutely so the
  `build.sh` call survives the `cd "$TITLE_DIR"` (a pre-existing path bug that
  blocked the host build + IR stages).

## Precision rules (verified against the oracle, see spec ledger M2-D1…M2-O1)

- **Multiplication** is exact at the summed fractional scale (floating).
- **FL division** keeps 13 significant digits, truncated; the integer part is
  always kept whole. Fixed modes round (5/4) or truncate (IN) to DP.
- **FL square root** fills to 14 significant digits (truncated); fixed modes
  round/truncate to DP. The root row carries no `^`.
- A mul/div/`%` result leaves its **rounded, displayed** value in `WR`.
- The `=`/`M=±` operand row prints at the entered scale for an entry-fed
  operation, else rescaled to DP.

## Known gaps

- Assumptions A1–A4 (M1) and the new M2 ledger rows M2-D1…M2-O1 are validated
  against the recovered-ROM transcripts but not yet re-derived line-by-line from
  the annotated listing; each ledger row names the routine to confirm against.
- The keyboard buffer, printer drum sequencing and lamp drivers (M3) do not
  exist; the core prints whole rows and exposes lamps directly.
- No target image: the `llvm-i4004` backend (M4) and the on-RTL run (M5) are out
  of scope for M2. `run_all.sh` correctly skips those stages.
