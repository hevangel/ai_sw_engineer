# 2026-10-22 — Busicom firmware fits 4 KB and runs 42/42 on the RTL (issue #18)

Branch `llvm/i4004-fit` (base fcc21e4). Uncommitted at the end of the
session; the user asked not to commit yet.

## Outcome

- Flat image **4,095 B of 4,096** (xl board, bank-0 RAM, no banking).
  SHA-256 `4c0a9535…8061`.
- **42/42 on the ai_hw_engineer RTL** (Verilator) and **42/42 on
  external/chips**. Both traces are identical to `oracle/rom_port_trace.txt`.
- Host `cargo test` green; `fmt` and `clippy -D warnings` clean.
- `run_tests.sh` green: lit 17 + 16, compares 11,180, nibble ops 7,440.
- `exec_num.py` 4,600/4,600.

## Work in this session (after task 3)

- Backend:
  - zero compares as `LD; JCN`
  - `i8` setcc results
  - constant multiply decomposition
  - disjoint `or` as `add`
  - MachineOutliner (JMS/BBL bodies, JUN tail form)
  - `getTypeForExtReturn` widens to `i8`
  - packer: page-filling order, PAGE8 skip relocations, JCN landing-page and
    0xFC fixes, self-call depth
- Firmware:
  - one digit per 4002 character
  - `asm/num.s` leaves and whole-operation entries (`__n_add/mul/div/sqrt/
    round/compute`)
  - `asm/drivers.s` (`__kb_scan`, `__pr_row`/`__pr_print`, `__bus_lamps`)
  - call graph flattened to 3 levels
- Bugs that only the target runs exposed:
  1. `operand_shown`'s `bool` return came back in P2:P3 (chips run).
  2. The printer fired one sector early on the RTL because the two
     simulators present TEST in opposite JCN phase.
  3. Sector-12 hammers merged into the next row on the RTL.

  The platform README's claim that the adapters compensate for TEST polarity
  was wrong and is corrected.
- Post-mortems: `failure_notes/2026-10-22-*` (seven notes).
- Reports: `size_history.md` rows, firmware and backend `final_report.md`,
  READMEs, the plan table.

## Not done

- Nothing committed; `bin/` image not added (waits for sign-off).
- Plan tasks C2 and C3 (nibble args, TTI) were not needed and not
  done.
- 1 B of ROM headroom.
