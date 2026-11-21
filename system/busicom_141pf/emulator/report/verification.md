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

---

# Verification - 2026-11-20 (debugger)

Added instruction-level debugging (issue #23, acceptance item "debugging
facilities"): `busicom_emulator::debug` and the `debug` CLI subcommand.
`advance` now runs through a shared `step_cycle`. Windows host, Rust/Cargo
1.99.1; original ROM SHA-256 `c6af325d...ceb7` (checked, 1280 bytes).

| Check | Result |
| --- | --- |
| `cargo test` (4 new `tests/debug.rs` tests plus existing) | Passed |
| `cargo test --test replay original_rom_replays_all_manual_examples -- --ignored` with `BUSICOM_ROM` | Passed (42 manual examples against the committed original-ROM trace) |
| Release CLI trace of `manual_examples.json` before vs after the refactor | Byte-identical (SHA-256 equal) |
| `cargo fmt -- --check`, `cargo clippy --all-targets -- -D warnings` | Passed |
| `busicom-emulator debug <original-rom> --steps 12 --dump` | Starts `CLB`, `JCN`, `JMS 0B0`, then the init loop; exit 0 |

The random-corpus replay (about 290 s in a debug build) was not rerun; the
refactor is covered by the identical manual-example trace. The debugger tests
use hand-assembled programs whose expected state follows the MCS-4 data sheet.
Limits: breakpoints stop after the instruction at the address ran, because the
pinned model exposes state only through its per-instruction debug log; no
watchpoints, disassembler or reverse stepping.

---

# Verification - 2026-11-20 (independent simulator)

Compared the emulator against `inside4004` (Python RTL-level MCS-4 model by
Patrick Leboutillier, CC BY-NC-SA 4.0, commit
`43c1a077c7fec2c7a3ff21e874f943ec41e00e61`, cloned outside the repository and
run, not copied). Its shipped `141-PF/ROM.bin` text converts to 1280 bytes
with SHA-256 `c6af325d...ceb7`, identical to the original ROM used here.

| Command | Result |
| --- | --- |
| `py platform/scripts/compare_inside4004.py --inside4004 <clone> --rom <original-rom> --emulator emulator/target/release/busicom-emulator.exe` | 42/42 examples identical, every printed row (digits, symbol column A, symbol column B), 164 s on 4 workers |

Method: each example's setup keys, selector changes and keystrokes are fed to
`inside4004` through its key buffer; the emulator runs the same example from
the committed schedule. Blank rows are ignored. This compares complete tapes,
stricter than the suffix rule of the committed oracle. Differences from the
simulator that the script handles: it has no diamond (`ST`) or `00` key (added
to its keyboard table at run time) and never clears the red state, so paper
colour is not compared.

The tapes were compared from independent code, but both run the same ROM, so
the check covers the emulator's board wiring, keyboard scan, printer decode and
pacing, not the firmware itself. Lamp state and cycle timing are not compared.

---

# Verification - 2026-11-20 (compiled Rust firmware on the emulator)

Added `Emulator::with_ram` and `--ram N` (2 to 16 4002 chips, bank `i / 4`
selected by DCL) so the emulator can run the image the Rust to LLVM to i4004
pipeline produces. Windows host, Rust/Cargo 1.99.1. The image was rebuilt from
this branch in `ai-sw-engineer:latest`
(`system/busicom_141pf/firmware/scripts/build_target_ir.sh`, then
`llvm-i4004/scripts/build_image.sh ... platform/link_xl.ld`):
`firmware.bin`, 4096 bytes, SHA-256
`4c0a9535d15735ab653066d84662dc7b3ffe84c142d9966e889755dfba678061`.

| Check | Result |
| --- | --- |
| `cargo test --test replay -- --include-ignored` with `BUSICOM_ROM` and `BUSICOM_FIRMWARE` | 5 passed. `compiled_rust_firmware_replays_all_manual_examples`: all 42 manual examples, rows and lamps identical to `rom_port_trace.txt` (the original-ROM oracle) |
| `busicom-emulator --ram 16 firmware.bin <manual schedule>` | 33 s; 600 non-comment trace lines identical to the oracle |
| Same three failing programs through `platform/scripts/run_image.sh secondary --board xl` (separate adapter on the pinned `chips`) | Output identical to the emulator's, line for line |
| 1000-program random corpus, compiled firmware, `--ram 16`, `compare_oracle.py` | **997/1000 pass; 3 differ** (R0534, R0568, R0872), 817 s release build |
| Same 3 programs on the original ROM | All three pass (expected tapes agree with the ROM) |
| New `xl_board_ram_banks_are_selected_by_dcl`, fmt, clippy `-D warnings` | Passed |

The three differences are compiled-firmware behaviours, not emulator ones: the
original ROM matches the expected tapes, and a second adapter reproduces the
compiled firmware's tape exactly. See
`failure_notes/2026-11-20-busicom-141pf-compiled-firmware-random-divergences.md`.
Both executors share the pinned `chips` cores, so this does not rule out a core
defect that affects only compiled code; the RTL run is the remaining check.
Not run: ROM-banked images (over 4096 bytes); the RTL primary executor.

---

# Verification - 2026-11-20 (R0534 debugging and printer drum-sync fix)

Debugged the three compiled-firmware random-corpus failures with the emulator
debugger (`failure_notes/2026-11-20-busicom-141pf-compiled-firmware-random-divergences.md`).
Per-key RAM of the compiled image matched the host core exactly, so the fault
was in printing; `hammer_trace` showed the first hammer of the failing row one
drum sector late, caused by a one-cycle race between the drum-index read and the
TEST-edge wait in `__pr_print`. Fixed in `firmware/asm/drivers.s`; the image
stays at 4096 bytes, SHA-256
`47772d01067bbcae341189b32f8d5f92ee2b574730aaf03a8567e5ecf0baa2fd`.

| Check | Result |
| --- | --- |
| The 3 failing programs vs the original ROM on the same schedule | Traces identical |
| `cargo test --release --test replay -- --include-ignored` with `BUSICOM_ROM` and `BUSICOM_FIRMWARE` | 5 passed (42 manual examples, original ROM and compiled firmware) |
| `platform/scripts/run_image.sh secondary --board xl` (separate adapter) on the fixed image, 42 examples | Tape equals the original-ROM oracle |
| 1000-program random corpus, compiled firmware, `--ram 16`, `compare_oracle.py` | **1000/1000** (4438 steps, 5325 rows, 1244 lamp checkpoints), 754 s |

Not run: the RTL primary executor on the fixed image, and a sweep that starts a
print at every cycle of a drum revolution (the deterministic test for this race).
