# Busicom 141-PF Rust emulator

Host Rust library and headless executable for the whole calculator: MCS-4
machine code, ROM/RAM, keyboard selectors and scan chain, drum printer and lamps.
This is separate from the rewritten calculator firmware and the LLVM backend.
Chip cores come from the pinned `external/chips`; board wiring is maintained here.

From the repository root:

```bash
git submodule update --init --depth 1 external/chips
cargo build --release --manifest-path system/busicom_141pf/emulator/Cargo.toml
python3 system/busicom_141pf/platform/scripts/build_schedule.py \
  system/busicom_141pf/firmware/oracle/manual_examples.json -o /tmp/busicom.schedule
cargo run --release --manifest-path system/busicom_141pf/emulator/Cargo.toml -- \
  /path/to/rom.bin /tmp/busicom.schedule > /tmp/busicom.trace
bash system/busicom_141pf/platform/scripts/compare.sh /tmp/busicom.trace
```

Supply your own original ROM (see
[provenance](../firmware/original/provenance.md)), or a compiled flat firmware
image. No original ROM is distributed by this crate. Flat ROMs up to 4096 bytes
are supported, with two stock RAM chips. XL RAM and banked images still use
the [platform runner](../platform/README.md). There is no graphical panel yet.

The library exposes `Emulator::new`, `reset`, `advance`, `lamps` and `replay`.
`advance` accepts selector values, a key code, and elapsed drum half-spins;
returned printer rows and `lamps()` can drive another host interface.
The CLI accepts the deterministic schedule documented in
[spec/spec.md](spec/spec.md), prints oracle-format output and exits nonzero on
invalid input. `--help` shows usage.

```bash
cargo test --manifest-path system/busicom_141pf/emulator/Cargo.toml
cargo fmt --manifest-path system/busicom_141pf/emulator/Cargo.toml -- --check
cargo clippy --manifest-path system/busicom_141pf/emulator/Cargo.toml --all-targets -- -D warnings
BUSICOM_ROM=/path/to/rom_141pf_combined.bin \
  cargo test --manifest-path system/busicom_141pf/emulator/Cargo.toml \
  --test replay original_rom_replays_all_manual_examples -- --ignored
```

The ROM-dependent test compares every payload line across all 42 manual
examples against the committed original-ROM trace. It is explicitly ignored in
normal runs because the ROM is not redistributable. This emulator and the old
secondary trace adapter share chip cores; their agreement is not independent
evidence. The oracle's prior RTL cross-check is recorded in the
[oracle provenance](../firmware/oracle/README.md). See
[testplan](plans/testplan.md) for the verification scope.

The [verification record](report/verification.md) records the completed
42/42 original-ROM replay and exact release-CLI trace comparison.
