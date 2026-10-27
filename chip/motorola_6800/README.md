# chip/motorola_6800

Native Rust MC6800 CPU core (issue
[ai_sw_engineer#264](https://github.com/hevangel/ai_sw_engineer/issues/264)).

`emulator/` is a `no_std`, bus-cycle-accurate MC6800 library: one `tick`
per bus clock, bus accesses on real cycles, IRQ level-sampled and NMI
edge-latched at instruction boundaries (and every cycle while waiting
after WAI). Status: **implemented and validated** — the full-ISA vector
replay (1,948 boundary records, clock-keyed IRQ/NMI triggers, final-memory
compare) and all 50 third-party hakanrw/mc6800 programs pass against the
independent oracle in `ai_hw_engineer/design/motorola_6800`, whose RTL
consumes the same fixtures.

| Path | Contents |
| --- | --- |
| `emulator/` | the core (`src/lib.rs`, `src/cpu.rs`) and the replay tests |
| `spec/isa.md` | ISA contract, semantics source, assumptions ledger |
| `plans/` | implementation plan and test plan |
| `oracle/` | committed fixtures with provenance: `isa/` (vector replay), `thirdparty/` (50 assembled programs + expected windows) |
| `report/final_report.md` | results, evidence, known deviations |

Run:

```sh
cargo test --manifest-path chip/motorola_6800/emulator/Cargo.toml
```

Related: RTL design [ai_engineer#43](https://github.com/hevangel/ai_engineer/issues/43),
toolchain [ai_engineer#61](https://github.com/hevangel/ai_engineer/issues/61).
