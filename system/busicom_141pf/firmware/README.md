# Busicom 141-PF calculator firmware (Rust)

| | |
|---|---|
| Original year | 1971 (Busicom 141-PF introduction; see [system README](../README.md)) |
| Original authors | Intel, for Busicom (firmware reverse-engineered by the Busicom 141-PF Replication Project) |
| Machine | Busicom 141-PF — Intel 4004, 5 × 4001, 2 × 4002, 3 × 4003 |
| Status | **M3** — host core (M2: 42/42) plus port-level machine drivers: keyboard 4003 scan + 8-key buffer, selectors, drum-printer hammer/`TEST` sequencing, paper advance, red/black ink, lamps, all behind a port-level HAL and proven **42/42** through a host board model validated against the original ROM |

## History

The 141-PF's 1280-byte firmware was the first commercial microprocessor program: a 14-digit printing calculator with subtotal and grand-total accumulators, one memory, constants, percentage and (on this ROM) square root, driving a drum printer through 4003 shift registers in real time from a 4-bit CPU. This title rewrites it in Rust and will compile it back to 4004 machine code through `llvm-i4004`.

## Layout

| Path | Contents |
|---|---|
| [spec/](spec/spec.md) | behavior spec, assumption ledger, [I/O contract](spec/io.md) |
| [original/](original/provenance.md) | ROM provenance and SHA-256 (binary not committed) |
| [oracle/](oracle/README.md) | 42 manual examples with original-ROM output; [`expected_pass.txt`](oracle/expected_pass.txt) + [`port_level_pass.txt`](oracle/port_level_pass.txt) ratchets; [`port_level_validation.md`](oracle/port_level_validation.md) + `rom_port_trace.txt` (M3) |
| `src/` | `bcd.rs` digit-serial BCD, `keys.rs`, `print.rs` rows, `lib.rs` calculator, `hal.rs` port-level board I/O, `driver_keyboard.rs` / `driver_printer.rs` / `driver_lamps.rs` M3 drivers, `main.rs` target/host entry |
| `tests/manual_examples.rs` | replays the oracle JSON through the core |
| `tests/port_level.rs` | replays the oracle JSON through the M3 drivers + a host board model (port-level) |
| [plans/](plans/implementation_plan.md) | milestones M0–M5, [test plan](plans/testplan.md) |
| [report/](report/final_report.md) | current results |

## Run

```bash
./scripts/docker_run.sh bash -c 'cd system/busicom_141pf/firmware && ./scripts/run_all.sh'
cargo test --test manual_examples -- --nocapture          # per-example PASS/FAIL
echo "dp=3 round=IN 1 2 . 3 4 + 3 4 . 5 6 - =" | ./scripts/run_host.sh
```
