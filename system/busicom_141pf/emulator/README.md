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
are supported, with 2 stock RAM chips by default or up to 16 (`--ram 16`, the
xl board the compiled firmware is linked for). ROM-bank-switched images (over
4096 bytes) still use
the [platform runner](../platform/README.md). There is no graphical panel yet.

The library exposes `Emulator::new`, `reset`, `advance`, `lamps` and `replay`.
`advance` accepts selector values, a key code, and elapsed drum half-spins;
returned printer rows and `lamps()` can drive another host interface.
The CLI accepts the deterministic schedule documented in
[spec/spec.md](spec/spec.md), prints oracle-format output and exits nonzero on
invalid input. `--help` shows usage.

## Running the compiled Rust firmware

The Rust to LLVM to i4004 image is a 4096-byte flat ROM linked for the xl board,
so run it with 16 RAM chips (`--ram 16`):

```bash
system/busicom_141pf/firmware/scripts/build_target_ir.sh && \
  llvm-i4004/scripts/build_image.sh system/busicom_141pf/firmware \
  system/busicom_141pf/firmware/build/target system/busicom_141pf/platform/link_xl.ld   # in ai-sw-engineer
busicom-emulator --ram 16 system/busicom_141pf/firmware/build/target/firmware.bin /tmp/busicom.schedule
BUSICOM_FIRMWARE=system/busicom_141pf/firmware/build/target/firmware.bin \
  cargo test --manifest-path system/busicom_141pf/emulator/Cargo.toml \
  --test replay compiled_rust_firmware_replays_all_manual_examples -- --ignored
```

All 42 manual examples reproduce the original-ROM tape. Of the 1000 random
programs the compiled image fails 3 (see the failure note in the verification
record).

Related documents: [machine contract](spec/machine_contract.md) (shared with the SV
system issue), [inventory of existing emulators](spec/inventory.md), and the
independent-simulator comparison `../platform/scripts/compare_inside4004.py`
(42/42 identical tapes against inside4004, see [verification](report/verification.md)).

## Debugger

`busicom-emulator debug <image.bin>` runs instructions from reset with fixed
selector/key inputs and prints one trace line per instruction: program
counter, opcode (and second byte), ACC, carry, TEST and R0..RF **before** the
instruction. Options: `--steps N`, `--skip N`, `--break ADDR` (repeatable, hex
with `0x` or decimal), `--precision`, `--rounding`, `--key`, `--dump` (4002
RAM characters/status/ports and 4001 ports at the end).

```bash
busicom-emulator debug /path/to/rom.bin --steps 20 --break 0x06e --dump
```

The same facilities are in the library (`busicom_emulator::debug`):
`enable_cpu_probe`, `step_instruction`, `run`, `run_to_breakpoint`,
`ram_char`, `ram_status`, `ram_port` and `rom_port`. A breakpoint stops
*after* the instruction at that address ran; the next step shows its effect.
The pinned 4004 model keeps its registers private, so CPU state is read from
the model's own per-instruction debug log through a thread-local probe. The
probe is off by default and slows stepping; plain replays are unaffected.

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

Beyond the manual examples, a seeded random corpus
([random_calculator_tests.json](../firmware/oracle/random_calculator_tests.json),
1000 mixed-operation programs) is cross-checked the same way: expected tapes
are computed by the firmware host core, which shares no code with this crate.

```bash
BUSICOM_ROM=/path/to/rom_141pf_combined.bin \
  cargo test --manifest-path system/busicom_141pf/emulator/Cargo.toml \
  --test random_replay random_corpus_replays_original_rom -- --ignored
# CLI equivalent for the same corpus (a few minutes):
py system/busicom_141pf/platform/scripts/build_schedule.py \
  system/busicom_141pf/firmware/oracle/random_calculator_tests.json \
  -o /tmp/busicom_random.schedule
system/busicom_141pf/emulator/target/release/busicom-emulator.exe \
  /path/to/rom_141pf_combined.bin /tmp/busicom_random.schedule > /tmp/busicom_random.trace
py system/busicom_141pf/platform/scripts/compare_oracle.py \
  /tmp/busicom_random.trace system/busicom_141pf/firmware/oracle/random_calculator_tests.json
```

The [verification record](report/verification.md) records the completed
42/42 original-ROM replay and exact release-CLI trace comparison.
