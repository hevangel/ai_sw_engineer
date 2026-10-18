# Bare-metal Rust for historical targets

## Crate shape (see `system/_template/software/`)

- `lib.rs`: `#![no_std]`, no allocation, logic only, behind a `Hal` trait.
- `main.rs`: `#![cfg_attr(target_os = "none", no_std, no_main)]`; host `fn main` uses std for interactive runs; the target path exports `extern "C" fn main() -> !` and a `#[panic_handler]`.
- Release profile: `opt-level = "z"`, `lto = true`, `codegen-units = 1`, `panic = "abort"`.

## Emitting LLVM IR

```bash
cargo rustc --release --lib -- --emit=llvm-ir -o build/host/<sw>.ll          # host IR, for reading
cargo +nightly rustc --release --bin <sw> --target llvm-<chip>/rust/<chip>-none.json \
    -Zbuild-std=core -Zbuild-std-features=panic_immediate_abort -- --emit=llvm-ir,asm
```

Target IR is what the backend must handle; host IR differs (pointer width, data layout). Always inspect the target IR when the backend misbehaves.

## Custom target JSON

- `data-layout` must equal the backend's `spec/abi.md` string exactly, or rustc refuses / miscompiles.
- `max-atomic-width: 0`, `singlethread: true`, `panic-strategy: abort` for these CPUs.
- `target-pointer-width` is 16 for 6502/8008/8086 real mode; the 4004 needs a decision in its ABI (12-bit ROM addresses, separate RAM/register spaces).
- Custom LLVM targets also need a rustc built against that LLVM (`rustc_target` must know the `arch`); `external/rust-mos` is the worked example.

## Pitfalls

- `core::fmt` is large; avoid `{}` formatting in target code — format digits by hand.
- 64-bit and 128-bit arithmetic pulls in big compiler-rt helpers; use the original's widths.
- `u16` multiply/divide become helper calls on 8-bit CPUs; count them in the size report.
