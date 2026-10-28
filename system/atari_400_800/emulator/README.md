# atari_400_800 emulator

Deterministic host Rust whole-system emulator for the Atari 400/800
([ai_sw_engineer#170](https://github.com/hevangel/ai_sw_engineer/issues/170)).
Library + headless CLI. The CPU is the pinned `external/mos6502` crate
(NMOS 6502 — the SALLY's core); this crate provides the Atari bus, ANTIC,
GTIA, POKEY and PIA models, the scanline/VBI timing loop and the CLI.

**Status: first working slice.** The CPU (every documented NMOS 6502
opcode, binary and decimal mode) passes the pinned Klaus Dormann functional
suite through this emulator's own bus. Machine coverage: address decode with
hardware mirrors, WSYNC/VCOUNT/NMIST/NMIRES and the vertical-blank NMI, the
POKEY IRQ/RANDOM model, PIA DDR semantics, 16K/48K profiles, 8K/16K
cartridges. **Not yet:** display-list execution and video output, audio,
serial/SIO/disk, input devices, PAL. Machine contract and gaps:
[spec/spec.md](spec/spec.md). Test plan: [plans/testplan.md](plans/testplan.md).

## Build and test

```sh
cd system/atari_400_800/emulator
cargo build
cargo test            # includes the Klaus Dormann run (~seconds)
cargo run -- --help
```

## Run

```sh
# Klaus Dormann 6502 functional suite on the bare test bus
cargo run -- --profile flat64k \
    --image ../../external/6502_65C02_functional_tests/bin_files/6502_functional_test.bin@0 \
    --pc 0400 --cycles 200000000 --dump-state
# → PC=$3469 ("test passed, no errors")

# A user-provided OS ROM booting 2 frames, headless and deterministic
cargo run -- --os-rom /path/to/os.rom --frames 2 --trace
# ROM/cartridge images are never distributed with the emulator (copyright);
# see spec/spec.md#copyright.
```

Options: `--profile atari800|atari400|flat64k`, `--image FILE@ADDR`
(repeatable), `--cart FILE` (8K/16K), `--os-rom FILE` (8K at $E000),
`--pc HEX`, `--frames N`, `--cycles N`, `--trace` (stderr), `--dump-state`.

## Library

```rust
use atari_400_800_emulator::{Machine, MachineProfile};

let mut m = Machine::new(MachineProfile::Atari800);
m.load(0x2000, &program);          // bytes through the bus decode
m.load_os_rom(&os_rom_image)?;     // 8 KiB at $E000-$FFFF
m.reset();                         // fetch OS ROM reset vector
m.run_frames(2);                   // 262 scanlines × 114 cycles per frame
```

Determinism: no wall clock, no host entropy — identical construction and
commands produce identical state.
