# The compiled Rust firmware fails 3 of the 1000 random corpus programs that the host core and the original ROM pass

- Area: system/busicom_141pf/firmware compiled through llvm-i4004 (Rust to
  LLVM IR to i4004 image); first run of the compiled image on the Rust system
  emulator with the random corpus
- Found by: replaying `oracle/random_calculator_tests.json` on the compiled
  `firmware.bin` (4096 bytes, SHA-256
  `4c0a9535d15735ab653066d84662dc7b3ffe84c142d9966e889755dfba678061`) on the
  emulator (`--ram 16`), scored by `platform/scripts/compare_oracle.py`
  (issue #23, 2026-11-20). The 42 manual examples pass 42/42 on the same
  image.

## What broke

997 of 1000 programs match. Three differ (all otherwise match on every earlier
step of the same program):

| Program | Switches | Step | Keys | Expected | Compiled firmware printed |
| --- | --- | --- | --- | --- | --- |
| R0534 | DP 6, round 5/4 | 4 | `3115 +`, `.3443 +`, `ST`, `=` | `3115.344300 *` | `4226.455411` with no `*` total symbol (the `ST` row printed the correct `3115.344300` first) |
| R0568 | DP 2, truncate | 9 | repeated `=` after `x 2.4` | `5.76 =` then `13.82 *` | `6.87` with the square-root symbol, then `13.82 *` |
| R0872 | DP 0, round 5/4 | 5 | `9621 /` | `9621 /` | `.732 M+` |

The expected tapes come from the firmware host core (the same Rust source built
for the host), and the original ROM reproduces all three programs, so the
divergence is introduced by building that source for the i4004.

## Root cause - and why our tests missed it

Not yet found. Candidates: a backend or runtime defect on a code path the
manual examples never reach (R0534 and R0568 are subtotal and constant
repetition paths), stale state across the banked software stack or frame
overlay, or a wide-integer libcall.

What is established: the Rust system emulator and the separate
`platform/scripts/chips_trace.rs` adapter print identical tapes for these
three programs, so the emulator wiring is not the cause. Both use the pinned
`chips` cores, so a core defect that only compiled code reaches is not ruled
out. The RTL primary executor (`run_image.sh primary --board xl`) has not been
run on them.

The compiled image had been checked only against the 42 manual examples
(`toolchain/verify.sh`), which the random corpus is built to extend.

## Fix and re-verification

Open. Next steps: run the three programs on the RTL; bisect by building the
firmware with fewer optimisations; step the compiled image with the emulator
debugger (`busicom-emulator --ram 16 debug`) to the first wrong digit.

## Prevention

`emulator/tests/replay.rs::compiled_rust_firmware_replays_all_manual_examples`
now runs the compiled image on the emulator (opt-in, `BUSICOM_FIRMWARE`). The
random corpus replay of the compiled image should become a gate of
`toolchain/verify.sh` once these three pass.
