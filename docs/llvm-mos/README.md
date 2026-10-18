# llvm-mos / rust-mos (6502 family)

Used by `llvm-mos6502` for Apple II/IIe (6502/65C02), Atari 2600 (6507), C64 (6510), NES (2A03).

In the Docker image:

```bash
rustc +mos --print target-list | grep mos      # mos-unknown-none + per-platform targets
ls $RUST_TARGET_PATH                            # mos-c64-none.json, mos-nes-*-none.json, mos-atari2600-*-none.json, ...
mos-c64-clang --version                         # llvm-mos SDK drivers per platform
cargo +mos build --release --target mos-c64-none -Zbuild-std=core
```

Notes:

- llvm-mos models zero page as "imaginary registers" (`rc0`..`rc31`); the SDK linker script for each platform reserves them.
- Each SDK platform supplies crt0, linker script and memory layout; our `system/<sys>/platform/link.ld` may wrap or replace it — record which in the platform README.
- The 2A03 has no decimal mode; Apple IIe Enhanced is a 65C02 (`-mcpu=mos65c02`). Pick the CPU flag per system, not per backend.
- Sources: [llvm-mos wiki](https://llvm-mos.org/wiki/Welcome), `external/llvm-mos-sdk/README.md`, `external/rust-mos/README.md`.
