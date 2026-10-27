# Rust for llvm-i8086

## The rustc with an i8086 target

`rustc` has a target `i8086-unknown-none` (tier 3 in rustc's terms, our fork): the nightly source the repository pins (rust-lang/rust 7df81381d, nightly-2026-10-15) plus `rust/patches/apply_i8086_target.py`, built against **our** LLVM. The patch adds the architecture (`Arch::I8086`, the target `i8086_unknown_none` with D1's data layout, 16-bit `c_int`, no atomics, `panic=abort`), the LLVM target initialisation (`LLVMInitializeI8086*`), the msp430-style calling convention for aggregates, `extern "msp430-interrupt"` as the interrupt ABI (the LLVM calling convention that `llvm-i8086` compiles as an interrupt handler, D5), LLVM's own `va_arg` for C-variadic functions, and no reliable `f128` (the backend has no fp128 soft-float). Nothing else of rustc changes.

Build, in the `ai-sw-engineer` image with the volume `ai-sw-llvm-build` (about 10 minutes for LLVM, about 15 for rustc and the libraries on 32 cores):

```bash
llvm-i8086/scripts/build_llvm.sh            # puts the overlay sources in the volume (once)
llvm-i8086/scripts/build_llvm_for_rust.sh   # LLVM libraries with X86 + I8086 and llvm-config
llvm-i8086/scripts/rust_build.sh            # fetches the pinned rust-lang/rust commit once, patches it, builds stage 1
```

The result is `/build/rust-obj/x86_64-unknown-linux-gnu/stage1/bin/rustc` with `core`, `alloc` and `compiler_builtins` built for `i8086-unknown-none` in its sysroot. `scripts/build_rust.sh` and `scripts/build_compiler_builtins.sh` use it when it exists (`$I8086_RUSTC`): `cargo +nightly rustc --target i8086-unknown-none -Z build-std=core`, `--emit=llvm-ir`, then `llc -mtriple=i8086` (the IR already has our triple and data layout; nothing is retargeted). `core` is rebuilt at the crate's opt-level, so the code sizes are the ones of the earlier route. The same sources through the new rustc give **byte-identical programs**: the Rust CHKDSK.COM has the same SHA-256 as the one built by the msp430 route, so every RTL recording of the repository still applies.

Finding this took two backend changes: the libm library calls (`fma`, `floor`, `fmin`, ... for `f32` and `f64`; `compiler_builtins` defines them) had no names in `initLibcallLoweringInfo`, which crashed the optimised compiler (`unsupported library call operation`), and rustc's tests of `bfloat`/`f128` support are answered "not reliable" for this target.

## The earlier route (no i8086 rustc)

Without `$I8086_RUSTC` the scripts take the msp430 route: rustc builds `core` and the crate for `msp430-none-elf` (`-Zbuild-std=core`, `panic=abort`, fat LTO), whose data layout equals `spec/abi.md` D1 for every alignment that matters, emits one LLVM IR module, and the script replaces its triple and data layout with ours, refuses what the backend lacks (atomics, vectors), and runs `llc -mtriple=i8086`.

Programs: `tests/smoke/hello_rs` and `tests/smoke/stress_rs` (one source, run on the 8088 and natively on the host, texts compared) with the HAL in `system/ibm_pc/platform/hal` and `runtime/{crt0,hal}.s`. Tests: `system/ibm_pc/emulator/tests/i8086_rust.rs` (bare core always; original BIOS with `--ignored`).

`tests/smoke/irq_rs` takes over the timer and keyboard interrupts with handlers written in Rust: `extern "msp430-interrupt" fn` (feature `abi_msp430_interrupt`) is the `msp430_intrcc` calling convention, which llvm-i8086 compiles as an interrupt handler (`spec/abi.md` D5); `ibm_pc_hal::set_vector` installs it. It counts five timer ticks, reads six scan codes the test types, and prints them. The RTL and the Rust emulator show the same screen (`system/ibm_pc/emulator/oracle/boot/irq_rs.txt`, `tests/boot_oracle.rs`; the final registers are not compared because the timer interrupt does not fall on the same instruction in both).

Variadic functions and `byval` arguments compile (`spec/abi.md` D3a), so `core::fmt` machinery that uses them, and C-style APIs, work. Port I/O has no intrinsic: it is assembly in the HAL (`runtime/hal.s`) or inline `asm` with explicit register constraints (`tests/codegen/inlineasm.ll`).

Floating point and `i128` work: `f32`, `f64` and 128-bit integers compile to the soft-float and 128-bit routines of `compiler_builtins` (`rt/obj/compiler_builtins.o`, abi.md D6); `tests/smoke/float_rs` prints the bit patterns of 69 lines of arithmetic, conversions and 128-bit operations, and the text is identical to what the host's hardware floating point gives (`system/ibm_pc/emulator/tests/i8086_rust.rs`).

Known limits: the builds that do not fit the 64 KiB tiny model are skipped by the matrix test, which says so: opt-level 0 (70 to 95 KB) and the `llc -O0` builds at opt-level 1 and 3. Everything else agrees with the host: `scripts/stress_matrix.sh` builds `stress_rs` at opt-level 0, 1, 2, 3, `s`, `z` with `llc -O2` and `-O0`; all twelve compile (opt-level 0 needed the half-precision and signed-multiply-with-overflow libcalls), eight of the twelve fit and run on the bare 8088 core and print the host's text. Finding this took two backend fixes: a displacement wider than 16 bits tripped an assertion when compiling `core`, and values pinned to the single-register class `GR16_BASE` could not be allocated (selection now makes a short-lived copy at each address use and `shouldCoalesce` refuses to join into the address classes).
