# Mission and pipeline

Read this when you start a new title, system or backend, or need to know what "done" means.

Build both the Rust/LLVM toolchain for each chip family and a runnable Rust
emulator for each complete system, alongside historical software reimplementations.
An LLVM backend or a host-only rewrite of a title does not replace the system
emulator. Reimplement historical software in Rust and run it on the original chip:

1. **Oracle** — run the original binary in a chip simulator and capture its behavior.
2. **Rust** — reimplement it as a `no_std` crate; prove it on the host against the oracle.
3. **LLVM IR** — `rustc --emit=llvm-ir` (or `cargo rustc -- --emit=llvm-ir`).
4. **Backend** — lower IR to the target ISA with `llvm-<chip>/` (llc → asm → assembler → linker).
5. **Target run** — run the image on the chip simulator and compare against the same oracle.
6. **System emulator** — maintain `system/<system>/emulator/`, a host Rust
   library and executable that run original and compiled machine-code images
   through the CPU, memory and peripheral models. Include keyboard/input,
   observable output, deterministic replay, specifications, tests and run commands.
   Reuse pinned chip cores where appropriate; keep system wiring and behavior in
   this repository and record dependencies and unsupported hardware explicitly.

Related: [simulators.md](simulators.md) (oracles and emulator rules), [repository.md](repository.md) (where each piece lives), [verification.md](verification.md) (what must pass before a PR).
