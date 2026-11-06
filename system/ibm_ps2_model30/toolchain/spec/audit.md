# Audit of compiler support for the IBM PS/2 Model 30, and pinned versions

Acceptance item 1 of [ai_sw_engineer#60](https://github.com/hevangel/ai_sw_engineer/issues/60), 2026-11-06.

## Upstream

The Model 30's CPU is the 8086, so the upstream findings of the 5150 profile apply unchanged ([../../../ibm_pc/toolchain/spec/audit.md](../../../ibm_pc/toolchain/spec/audit.md)): LLVM 23.1.3 has no 8086 target (X86's 16-bit mode emits 80386 code), rustc nightly-2026-10-15 has no 16-bit x86 target, and the 16-bit C compilers outside LLVM (gcc-ia16, Open Watcom, Turbo C) do not compile Rust and are not part of this profile.

## Emulators of the Model 30 (for the execution gate)

| Emulator | Model 30 (8086)? | Notes |
|---|---|---|
| MartyPC (`external/martypc`, `05c0d088`) | no | 5150, 5160, PCjr, Tandy 1000 |
| MAME 0.264 (Ubuntu 24.04 package) | no | `i8530286` (Model 30-286), `i8530h31` and later PS/2s; checked with `mame -listfull` |
| 86Box | yes | `[8086] IBM PS/2 model 30` (`ibmps2_m30`, `src/machine/machine_table.c`), BIOS revisions 0-2 from its ROM set; an automation socket in the development branch (`doc/specifications/automation.md`) |

## Project-owned, reused as is

The backend (`llvm-i8086`), runtime library, `i8086-ld` (now with `--floppy-kib 720`), the shared HAL and DOS start-up (`system/ibm_pc/platform`), `pcdisk` (now with 720 KiB disks), and the independent checks (`system/ibm_pc/toolchain/scripts/check_images.py`, `mutants.py`). New here: `platform/runtime/crt0.s` (720 KiB boot), `platform/link.ld`, `platform/runtime/ps2.s`, `smoke/ps2_info_rs`.

## Pinned versions (the run of the report)

Same as the XT profile ([../../../ibm_pc_xt_5160/toolchain/spec/audit.md](../../../ibm_pc_xt_5160/toolchain/spec/audit.md)): LLVM 23.1.3 `0d261d1ca552` built by `llvm-i8086/scripts/build_llvm.sh`, rustc nightly-2026-10-15 (`7df81381d`) with `rust-src`, cargo 1.99.1, GNU binutils 2.42, Capstone 5.0.7. Original software: 86Box ROM set `c4cd8da9` (BIOS), pcjs-diskettes `cd343374` (PC DOS 3.30).
