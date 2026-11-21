# Emulator verification

1. Reject empty/oversized images and malformed schedules with useful errors.
2. Execute small hand-encoded 4004 programs through real port instructions to
   check ROM input, RAM lamp output and reset isolation, rather than calling
   firmware's calculator logic.
3. With the user-supplied original ROM, replay all 42 committed manual examples
   and compare every trace payload line against `rom_port_trace.txt`. This
   opt-in test requires `BUSICOM_ROM`; normal tests never need copyrighted ROMs.
4. With the user-supplied original ROM, replay the seeded random corpus
   (`firmware/oracle/random_calculator_tests.json`, 1000 mixed-operation
   programs) and check every step's expected tape suffix and lamps. The
   expected rows come from the firmware host core
   (`firmware/tests/random_calculator_tests.rs`), an implementation that
   shares no code with this crate, so agreement cross-checks the emulator and
   the original firmware against an independent model. Opt-in for the same
   ROM reason; a debug run takes a few minutes.
5. Debugger (`tests/debug.rs`): step hand-encoded programs and check PC, ACC,
   registers, opcode/operand bytes, two-cycle instruction handling, breakpoints
   and RAM/port reads against MCS-4 data sheet semantics (LDM, XCH, JUN, FIM,
   FIN, SRC, WMP). Stepping shares `advance`'s cycle path; the original-ROM
   replay (item 3) and an exact trace diff before and after the refactor
   guard that path.
6. Run host format and clippy checks. Build the CLI and exercise schedule input
   and error exit status. Firmware and LLVM sources are unchanged.

The exact original-ROM regression is the behavior gate. This core also drives
the old secondary adapter; independent evidence is the RTL validation recorded
in the oracle provenance, not a claim that two wrappers are independent.

7. Independent simulator: with a local clone of inside4004 and the original ROM, run `platform/scripts/compare_inside4004.py`. It replays every manual example through that simulator and requires the complete printed tape to match the emulator's row for row (see report/verification.md and spec/inventory.md).

8. Compiled Rust firmware: build the xl-board image (`toolchain/README.md`), then run `compiled_rust_firmware_replays_all_manual_examples` with `BUSICOM_FIRMWARE` (16 RAM chips, rows and lamps must equal the original-ROM oracle) and replay the random corpus with `--ram 16` through `compare_oracle.py`. The corpus run passes 1000/1000 since the drum-sync fix; the race it fixed is described in `failure_notes/2026-11-20-busicom-141pf-compiled-firmware-random-divergences.md`.
