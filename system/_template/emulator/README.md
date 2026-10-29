# [SYSTEM] Rust system emulator

Host Rust library and headless executable for the whole [SYSTEM] machine:
[CHIP] machine code, memory and peripherals. Separate from the title
reimplementations and the `llvm-[CHIP]` backend. Status: template, no CPU core yet.

From the repository root:

```bash
cargo build --release --manifest-path system/[SYSTEM]/emulator/Cargo.toml
cargo run --release --manifest-path system/[SYSTEM]/emulator/Cargo.toml -- \
  /path/to/image.bin system/[SYSTEM]/emulator/oracle/transcripts/000_placeholder.txt
```

Original images are not distributed; see each title's `original/provenance.md`.

```bash
cargo test --manifest-path system/[SYSTEM]/emulator/Cargo.toml
cargo fmt --manifest-path system/[SYSTEM]/emulator/Cargo.toml -- --check
cargo clippy --manifest-path system/[SYSTEM]/emulator/Cargo.toml --all-targets -- -D warnings
```

- [spec/spec.md](spec/spec.md): machine contract, dependencies, unsupported hardware, assumption ledger
- [plans/testplan.md](plans/testplan.md): verification levels and commands
- [report/verification.md](report/verification.md): dated verification record
