# Busicom 141-PF calculator firmware (Rust)

| | |
|---|---|
| Original year | 1971 (Busicom 141-PF introduction; see [system README](../README.md)) |
| Original authors | Intel, for Busicom (firmware reverse-engineered by the Busicom 141-PF Replication Project) |
| Machine | Busicom 141-PF — Intel 4004, 5 × 4001, 2 × 4002, 3 × 4003 |
| Status | **M5 done (2026-10-22, issue #18)**: the Rust firmware, compiled through `llvm-i4004` with hand-written BCD and driver routines in `asm/`, links to a flat **4,095 B** ROM (4,096 B budget, xl board, bank-0 RAM) and prints the original ROM's tape exactly, **42/42 on the ai_hw_engineer RTL and on external/chips**. Host tests 42/42. See `report/final_report.md` and `report/size_history.md`. |

## History

The 141-PF's 1280-byte firmware was the first commercial microprocessor program: a 14-digit printing calculator with subtotal and grand-total accumulators, one memory, constants, percentage and (on this ROM) square root, driving a drum printer through 4003 shift registers in real time from a 4-bit CPU. This title rewrites it in Rust and compiles it back to 4004 machine code through `llvm-i4004` (4,095 B, 3.2× the original).

## Layout

| Path | Contents |
|---|---|
| [spec/](spec/spec.md) | behavior spec, assumption ledger, [I/O contract](spec/io.md) |
| [original/](original/provenance.md) | ROM provenance and SHA-256 (binary not committed) |
| [oracle/](oracle/README.md) | 42 manual examples with original-ROM output; [`expected_pass.txt`](oracle/expected_pass.txt) + [`port_level_pass.txt`](oracle/port_level_pass.txt) ratchets; [`port_level_validation.md`](oracle/port_level_validation.md) + `rom_port_trace.txt` (M3) |
| `src/` | `lib.rs` calculator (key dispatch, state flags), `bcd.rs` BCD on 4002 registers (`Num` handles; entry externs to `asm/num.s` on the target, Rust twins on the host), `ram.rs` 4002 RAM access, `keys.rs`, `print.rs` rows and the print register, `hal.rs` port-level board I/O, `driver_keyboard.rs` / `driver_printer.rs` / `driver_lamps.rs` drivers (target overrides call `asm/drivers.s`), `main.rs` target run loop / host CLI |
| `asm/` | hand-written 4004 code linked into the image: `num.s` (BCD leaves `__leaf_*` and whole operations `__n_add/mul/div/sqrt/round/compute`) and `drivers.s` (`__kb_scan`, `__pr_row`/`__pr_print`, `__bus_lamps`) |
| `examples/num_twin.rs` | the Rust twins of the `asm/num.s` routines, driven by `scripts/exec_num.py` |
| `tests/bcd_equivalence.rs` | BCD property tests: exact decimal arithmetic, regressions, and the RAM-register core against the pre-issue-#18 core (`tests/legacy/`) |
| `tests/manual_examples.rs` | replays the oracle JSON through the core |
| `tests/port_level.rs` | replays the oracle JSON through the M3 drivers + a host board model (port-level) |
| `tests/port_trace.rs` | replays all 42 examples through the drivers + board model and compares the full printed tape (every row's value + both symbol columns + red, and the full 4-bit lamp nibble per step) **exactly** against the original ROM's `oracle/rom_port_trace.txt` |
| `scripts/` | `build_target_ir.sh` (Rust → one i4004 IR module), `exec_num.py` (asm routines vs twins on chips), `gen_worklist.py` (size gate), `run_all.sh`, `run_host.sh` |
| [report/size_history.md](report/size_history.md) | size per landing (`.text`, image, largest function) |
| [plans/](plans/implementation_plan.md) | milestones M0–M5, [test plan](plans/testplan.md) |
| [report/](report/final_report.md) | current results |

## Run

```bash
./scripts/docker_run.sh bash -c 'cd system/busicom_141pf/firmware && ./scripts/run_all.sh'
cargo test --test manual_examples -- --nocapture          # per-example PASS/FAIL
echo "dp=3 round=IN 1 2 . 3 4 + 3 4 . 5 6 - =" | ./scripts/run_host.sh
```

Target image (inside the `ai-sw-engineer` container, from the repo root):

```bash
bash system/busicom_141pf/firmware/scripts/build_target_ir.sh
bash llvm-i4004/scripts/build_image.sh system/busicom_141pf/firmware system/busicom_141pf/firmware/build/target system/busicom_141pf/platform/link_xl.ld
bash system/busicom_141pf/platform/scripts/run_image.sh secondary system/busicom_141pf/firmware/build/target/firmware.bin /tmp/out.txt --board xl
bash system/busicom_141pf/platform/scripts/compare.sh /tmp/out.txt
```

`run_image.sh primary` runs the same image on the ai_hw_engineer RTL; it needs Verilator (the `ai-hw-engineer` image, with `../ai_hw_engineer` mounted beside the repo).
