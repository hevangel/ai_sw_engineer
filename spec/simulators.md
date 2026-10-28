# Simulators, oracles and system emulators

Read this before capturing oracle data, running an image on a simulator, or touching `system/<system>/emulator/`.

- Deliver a Rust system emulator for every implemented system in `system/<system>/emulator/`, in addition to the per-chip LLVM/Rust backend and target software. Host emulator crates may use `std`; the target `no_std` rule does not apply to them.
- Start a new emulator from `system/_template/emulator/` (created by `./scripts/new_system.sh`). The template is a working crate with a placeholder echo core, a replay test and the spec/testplan/verification documents; replace the core, do not restructure the crate.
- Run both original and generated images, expose real machine I/O, and replay deterministic inputs against captured original-software output. Document hardware variants, timing approximations and gaps in `emulator/spec/spec.md` and `emulator/plans/testplan.md` before implementation, and keep them current.
- An emulator sharing a chip core with a reference adapter is not an independent oracle. Cross-check against RTL or another independent implementation; never derive expected output from the emulator under test.
- Primary oracle: the RTL and ISS in `../ai_hw_engineer` (`$AI_HW_ENGINEER`). Use its `system/<system>/` builds for whole-machine runs.
- Secondary oracle: an independent third-party emulator in `external/` (MAME, AppleWin, Stella, MartyPC, ...). When the two disagree, the disagreement is a finding, not noise.
