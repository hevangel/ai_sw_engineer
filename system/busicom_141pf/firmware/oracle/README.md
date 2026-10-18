# Busicom 141-PF firmware oracle

## `manual_examples.json`

All 42 numbered examples of the *Unicom 141 Series Operating Instructions* as key/switch sequences with expected printed rows and lamps (schema v1).

- Copied unchanged from `ai_hw_engineer/system/busicom_141pf/spec/reference/Unicom_141P_examples.json` at commit `e0ff8202f0f482f25c274785affa2336d6566a97` (2026-09-27). SHA-256 `1d36cd522e30b4cd7280907a43a202dc8f691719f597ca01ef10d63493608e1e`.
- Two expectation profiles per checkpoint:
  - `expect` — the scanned manual;
  - `expect_profiles.recovered-rom` — overrides where the **original ROM** prints something else (5 examples: 3-1, 5-1, 11-1, 11-2, 11-3).
- **This title's oracle is the recovered-ROM profile**: it is what the original 1280-byte firmware prints, observed independently on the ai_hw_engineer MCS-4 RTL (xezim and Verilator) and on V. Ilmer's `chips` emulator (`external/chips`), 42/42 examples, 135 checkpoints (ai_hw_engineer `report/manual_regression.md`).

Replay semantics (identical to ai_hw_engineer `scripts/replay_manual.py`): before each example set the switches and press `setup_keys` (`CE C CM C`); for each step apply `switches`, press `keys`, then require that the rows printed during the step **end with** `expect.tape`, and that the named lamps match. Rows are `{value, symbol, red, rounded}`; `value` keeps exact digits and decimal point, `null` with symbol `overflow` is the dotted overflow line.

Rounding switch values: `0` = FL (float), `1` = 5/4 (round), `8` = IN (truncate).

## Re-capture

```bash
cd "$AI_HW_ENGINEER/system/busicom_141pf"
sh scripts/run_manual_test.sh                       # RTL, original ROM
python3 scripts/check_reference.py --chips <external/chips> --profile recovered-rom
```
