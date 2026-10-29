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

---

# Verification — 2026-10-25 (random corpus)

Added the seeded random calculator corpus (issue #23). Same Windows host,
Rust/Cargo 1.99.1 (GNU); `py` is CPython 3.13. Original ROM as above
(SHA-256 verified before the runs). No firmware or emulator production code
changed; this adds corpus data, tests, the generator and the comparator.

| Command | Result |
| --- | --- |
| `py system/busicom_141pf/platform/scripts/gen_random_tests.py` | Wrote `firmware/oracle/random_calculator_tests.json` (1000 examples, 4852 steps, 19331 key presses) from seed 20261025 |
| `RANDOM_ORACLE_OUT=… cargo +1.99.1 test --manifest-path system/busicom_141pf/firmware/Cargo.toml --test random_calculator_tests dump_random_oracle -- --ignored` | Expected tapes and lamps for 4438 steps computed by the firmware host core and merged into the committed JSON |
| `cargo +1.99.1 test --manifest-path system/busicom_141pf/firmware/Cargo.toml` | All firmware suites pass, including `random_examples_replay` (1000 examples) and the 42-example manual/port suites |
| `py system/busicom_141pf/platform/scripts/gen_random_tests.py --check` | Programs match the seed; corpus not drifted |
| `py system/busicom_141pf/platform/scripts/build_schedule.py …random_calculator_tests.json -o build/busicom_random.schedule` | Deterministic schedule (31,183 lines) |
| `busicom-emulator.exe <original-rom> build/busicom_random.schedule > build/busicom_random.trace` (release) | Completed; 20,471 payload lines |
| `py system/busicom_141pf/platform/scripts/compare_oracle.py build/busicom_random.trace …random_calculator_tests.json` | **PASS: 1000/1000 examples (4438 steps, 5325 rows, 1244 lamp checkpoints)** |
| `cargo +1.99.1 test --manifest-path system/busicom_141pf/emulator/Cargo.toml --test random_replay random_corpus_replays_original_rom -- --ignored` with `BUSICOM_ROM` set | Passed (debug build, 288 s) |
| `cargo +1.99.1 fmt --…emulator/Cargo.toml -- --check` and `cargo +1.99.1 clippy --…emulator/Cargo.toml --all-targets -- -D warnings` | Passed (chips documentation warnings as above) |
| `cargo +1.99.1 fmt --…firmware/Cargo.toml -- --check` and `cargo +1.99.1 clippy --…firmware/Cargo.toml --all-targets -- -D warnings` | Passed |

Scope and limits. The corpus programs are composed only from behaviour
anchored by the 42 manual examples and the committed ROM port trace; the
generator documents its exclusions. The first, unconstrained corpus draft
diverged from the ROM in 88 steps across five firmware-core behaviours (FL
result width, width-14 false overflow, zero-result scale, sqrt of a recalled
radicand, `M±` alignment) — recorded in
`failure_notes/2026-10-25-busicom-141pf-random-corpus-divergences.md`, not
fixed here; the exclusions shrink as the core is corrected. `SIGN`, the `00`
key, entries with more decimals than DP and FL totals after `+`/`-` remain
uncovered (not oracle-anchored). The emulator again ran the original image,
not a newly compiled firmware image.
