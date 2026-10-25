# Verification — 2026-10-24

Verified on Windows with Rust/Cargo 1.99.1 (GNU toolchain); schedule generation
and the existing Bash comparator ran through Ubuntu 24.04 in WSL.

From the ai_sw_engineer root, with the pinned `external/chips` initialized:

| Command | Result |
| --- | --- |
| `cargo +1.99.1 test --manifest-path system/busicom_141pf/emulator/Cargo.toml` | 3 passed; ROM-dependent test explicitly ignored |
| `cargo +1.99.1 test --manifest-path system/busicom_141pf/emulator/Cargo.toml --test replay -- --include-ignored` with `BUSICOM_ROM` set | 4 passed, including exact replay of all 42 original-ROM manual examples |
| `cargo +1.99.1 fmt --manifest-path system/busicom_141pf/emulator/Cargo.toml -- --check` | Passed |
| `cargo +1.99.1 clippy --manifest-path system/busicom_141pf/emulator/Cargo.toml --all-targets -- -D warnings` | Passed for the emulator; the unmodified chips dependency emits existing documentation warnings |
| `cargo +1.99.1 build --release --manifest-path system/busicom_141pf/emulator/Cargo.toml` | Passed |
| `python3 system/busicom_141pf/platform/scripts/build_schedule.py system/busicom_141pf/firmware/oracle/manual_examples.json -o build/emulator.schedule` | Schedule generated with the existing builder (via WSL) |
| `system/busicom_141pf/emulator/target/release/busicom-emulator.exe <original-rom> build/emulator.schedule > build/emulator.trace` | Passed |
| `bash system/busicom_141pf/platform/scripts/compare.sh build/emulator.trace` | Passed: 42/42 examples, 846 payload lines identical (via WSL) |
| Release executable `--help`, then invocation without arguments | Help exits 0; invalid invocation exits 1 with usage on stderr |
| `git diff --check` | Passed |

Original ROM read from the sibling hardware checkout, SHA-256:
`c6af325d64b0895a8c6f7298e5443bbf5814783fabd6a1bcd2df08a9bec2ceb7`.
No binary was added to this repository. The secondary core remains pinned at
`1bc3d5781474ef9d7e7305af91071a723797f222`; no submodule pin changed.

RTL and compiler/target-image builds were not rerun: this change adds a host
emulator and guidance, without changing the firmware, ABI, LLVM backend or RTL.
Generated-image loading is supported; this run verifies the original image,
not a newly compiled firmware image. XL RAM, banked ROM and a graphical panel
remain outside this emulator's implemented scope.
