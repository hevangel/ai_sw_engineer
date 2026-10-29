# chip/motorola_6809

Native Rust MC6809 CPU core (issue
[ai_sw_engineer#265](https://github.com/hevangel/ai_sw_engineer/issues/265)).

`emulator/` is a `no_std`, bus-cycle-accurate MC6809 library: one `tick`
per bus clock, micro-op instruction sequences with data-sheet cycle
totals, IRQ/FIRQ level-sampled and NMI edge-latched at instruction
boundaries (NMI disarmed until LDS/LEAS loads S). Status:
**implemented and validated** — the unmodified reyco2000 functional,
interrupt and cycle-count suites all pass (exit 0); the cycle suite
verifies the data-sheet cycle count of every instruction form with zero
mismatches.

| Path | Contents |
| --- | --- |
| `emulator/` | the core (`src/lib.rs`, `src/cpu.rs`) and the suite harness (`tests/`) |
| `spec/isa.md` | ISA contract, semantics source, assumption ledger |
| `plans/` | implementation plan and test plan |
| `oracle/reyco2000/` | the GPL-3.0-licensed reyco2000/6809_6309_code_tester programs, listings and license (provenance in the README there) |
| `report/final_report.md` | results, evidence, timing comparison with the RTL |

Run:

```sh
cargo test --manifest-path chip/motorola_6809/emulator/Cargo.toml
```

Related: RTL design [ai_engineer#46](https://github.com/hevangel/ai_engineer/issues/46),
toolchain [ai_engineer#62](https://github.com/hevangel/ai_engineer/issues/62).
