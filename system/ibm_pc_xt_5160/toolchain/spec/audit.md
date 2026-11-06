# Audit of compiler support for the IBM PC XT 5160, and pinned versions

Acceptance item 1 of [ai_sw_engineer#59](https://github.com/hevangel/ai_sw_engineer/issues/59), redone 2026-11-06 now that the backend exists. The upstream findings are those of the 5150 profile ([../../../ibm_pc/toolchain/spec/audit.md](../../../ibm_pc/toolchain/spec/audit.md)), which apply unchanged: the XT has the same 8088.

## Upstream (nothing usable for an 8088)

- LLVM 23.1.3 has no 8086 target; X86's 16-bit mode is 80386 code with operand-size prefixes (clang `-m16`).
- rustc nightly-2026-10-15 has no 16-bit x86 target (the x86 targets are 32-bit `i386`/`i586`/`i686`).
- Outside LLVM and rustc there are 16-bit C compilers (gcc-ia16, Open Watcom, Turbo C, bcc); none compiles Rust, none is part of this profile, none was built for this audit.

## Project-owned, reused as is

| Component | Where | Notes |
|---|---|---|
| LLVM backend `I8086` | `llvm-i8086/target`, overlay on `external/llvm-project` | Apache-2.0 WITH LLVM-exception |
| Runtime library | `llvm-i8086/rt` (committed objects) | 32/64-bit arithmetic, memory functions, `compiler_builtins` |
| Linker `i8086-ld` | `llvm-i8086/asm` | now also writes 180, 320, 360 and 720 KiB floppies (`--floppy-kib`); default 160 |
| Platform | `system/ibm_pc/platform` | `crt0`, `crt0_com`, `dos`, `hal`, `ibm_pc_hal`, three linker scripts |
| `pcdisk` | `system/ibm_pc/platform/tools/pcdisk` | now handles the 160, 180, 320, 360 and 720 KiB DOS floppies |
| Independent checks | `system/ibm_pc/toolchain/scripts/check_images.py`, `mutants.py` | binutils + Capstone + llvm-objdump, relocations, placement |
| MartyPC harness | `system/ibm_pc/emulator/tools/martypc_oracle` | `MARTY_MACHINE=5160` added |

## Pinned versions (measured 2026-11-06, the run of the report)

| Tool | Version |
|---|---|
| LLVM (`external/llvm-project`) | 23.1.3, commit `0d261d1ca552`, built with `llvm-i8086/scripts/build_llvm.sh` (I8086 only, assertions on, gcc 13.3.0) |
| rustc (front end, msp430 route) | `rustc 1.101.0-nightly (7df81381d 2026-10-14)` = nightly-2026-10-15, with `rust-src` |
| rustc / cargo (host tools) | 1.99.1 (`58cf8f9c3`) |
| GNU binutils (independent decoder) | 2.42 (Ubuntu 24.04) |
| Capstone (independent decoder) | 5.0.7 (PyPI) |
| MartyPC (execution) | `external/martypc` `05c0d088e84ad6bbfac9b3f0d051e7eadefd9f44`, MIT |
| Original software | PCjs `1a91d38e` (XT BIOS 1982-11-08, BASIC C1.10), pcjs-diskettes `cd343374` (PC DOS 2.00) |

Acquisition: `git submodule update --init --depth 1 external/llvm-project external/martypc`, `llvm-i8086/scripts/build_llvm.sh` (WORKSPACE, BUILD_ROOT and JOBS can point it at a host build), `rustup toolchain install nightly-2026-10-15 -c rust-src`, `pip install capstone==5.0.7`, `scripts/fetch_reference.py DIR`.

## What this issue owns

Only the XT side: the profile ([profile.md](profile.md)), the XT smoke program, the build and run scripts, the expectations and the report. The backend, its target and the platform are not duplicated.
