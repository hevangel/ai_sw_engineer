# Busicom 141-PF firmware — test plan

| Level | What | Oracle | Command |
|---|---|---|---|
| Unit | BCD add/sub/shift/round, row formatting | hand-checked values | `cargo test --lib` |
| Manual replay | 42 examples, 135 checkpoints, recovered-ROM profile | original ROM on RTL + chips emulator (`oracle/manual_examples.json`) | `cargo test --test manual_examples -- --nocapture` |
| Port-level (M3) | drivers + core against the MCS-4 board model | same JSON; original ROM on `external/chips` (`oracle/port_level_validation.md`) | `cargo test --test port_level -- --nocapture` |
| Target (M5) | 4004 image on ai_hw_engineer RTL board | same JSON via `replay_manual.py` | `scripts/run_sim.sh` |

Ratchet: `oracle/expected_pass.txt` lists examples that must pass. The replay test fails if any listed example fails, and prints every example's status so progress is visible. Add IDs as milestones land; never remove one without a failure note.

Port-level ratchet: `oracle/port_level_pass.txt` is the same list for the M3 port-level replay (`tests/port_level.rs`); both are at 42/42.
