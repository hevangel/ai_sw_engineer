# Rust conventions

Read this before writing or changing Rust in a title or in a system emulator.

- Edition 2021, `#![no_std]` for every crate that reaches a target; `alloc` only when the platform provides a heap.
- Host system-emulator crates (`system/<system>/emulator/`) never reach a target, so they may use `std`.
- Portable logic in `lib.rs` behind a HAL trait; the host test and the target entry both implement the HAL.
- No floating point, no `u64`/`u128` on 4/8-bit targets unless the original used multi-precision math; prefer the widths the original used (BCD nibbles, 8-bit registers).
- Size matters: build target images with `opt-level = "z"`, `panic = "abort"`, `lto = true`, `codegen-units = 1`; report the size against the ROM budget.
- `cargo fmt` and `cargo clippy -- -D warnings` clean on the host build.
