# Busicom 141-PF firmware — test plan

Expected behaviour comes from the original ROM, never from this crate's own
Rust. The original's tape for the 42 manual examples
(`oracle/rom_port_trace.txt`) was captured on external/chips. The same ROM
scores 42/42 on the ai_hw_engineer RTL, on both the stock and the xl board.

| Level | What | Oracle | Command |
|---|---|---|---|
| Unit | BCD primitives, key buffer, lamps, row formatting | hand-checked values | `cargo test --lib` |
| BCD properties | `add`/`mul`/`div`/`sqrt`/`round` on random operands; division truncation against exact decimal arithmetic; regressions (division scale shortfall); the new RAM-register core against the pre-issue-#18 core (`tests/legacy/bcd_old.rs`) | exact arithmetic + the old core | `cargo test --test bcd_equivalence` |
| Manual replay | 42 examples, 135 checkpoints, recovered-ROM profile | `oracle/manual_examples.json` | `cargo test --test manual_examples -- --nocapture` |
| Port level (M3) | drivers + core on the host board model | same JSON | `cargo test --test port_level -- --nocapture` |
| Exact tape (M3) | every printed row (value, both symbol columns, red) and the 4-bit lamp nibble per step | `oracle/rom_port_trace.txt` | `cargo test --test port_trace` |
| Assembly routines (issue #18) | each `asm/num.s` routine linked by the real packer and run on external/chips with random inputs, against its Rust twin (`examples/num_twin.rs`) | the Rust twin (itself held to exact arithmetic above) | `python3 scripts/exec_num.py 200` (4,600 cases) |
| Target image, secondary (M5) | the 4,095 B image runs the 42 examples on external/chips (xl board) | `oracle/rom_port_trace.txt`, exact | `../platform/scripts/run_image.sh secondary build/target/firmware.bin out.txt --board xl` then `../platform/scripts/compare.sh out.txt` |
| Target image, primary (M5) | the same image on the ai_hw_engineer RTL (Verilator, `ai-hw-engineer` image) | same, exact | `run_image.sh primary … --board xl` + `compare.sh` |
| Size | `.text` per function and total; image vs 4,096 B | budget | `scripts/gen_worklist.py` (run by `llvm-i4004/scripts/run_tests.sh`); the packer fails the link over budget or over 3 call levels |

Rules:

- **Both simulators.** Any change to `asm/drivers.s` or other timing code is
  verified on the RTL and on chips. They present TEST in opposite phase, so a
  pass on one says nothing about the other
  (`failure_notes/2026-10-22-busicom_141pf-drum-phase.md`).
- **Assembly changes.** A change to `asm/num.s` reruns `exec_num.py`. A change
  that affects placement (any size change) reruns it too, because packer
  relaxation depends on page offsets.
- **Ratchets.** `oracle/expected_pass.txt` and `oracle/port_level_pass.txt` list
  the examples that must pass (both 42/42). Never remove an ID without a
  failure note.

Known gaps: type-ahead while the target is busy is not tested (spec M3-K3).
The full-image tests cover only the 42 manual examples.
