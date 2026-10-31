# Toolchain audit: IBM PC XT 5160 (Intel 8088)

Acceptance box 1 of ai_sw_engineer#59. Audited 2026-10-28. Distinguishes
upstream support from missing project-owned work. Versions are pinned where
measured in this run; "pin at implementation" marks values to fix when the
backend (ai_engineer#60) and bring-up start consuming them.

## Pinned project toolchain (measured)

From the repo `Dockerfile` and the host (host rustc matches the image):

| Tool | Version | Source |
|---|---|---|
| rustc stable (host builds/tests) | 1.99.1 | `Dockerfile` `RUST_STABLE`, host `rustc --version` |
| rustc nightly (`-Zbuild-std`, `rust-src`) | nightly-2026-10-15 | `Dockerfile` `RUST_NIGHTLY` |
| lit (backend test driver) | 18.1.8 | `Dockerfile` pip pin |
| Base image (rust-mos, 6502 work) | `mrkits/rust-mos:13f2838f9-334fc98-8f3a80f8` | `Dockerfile` |
| LLVM base for new backends | branch `llvmorg-23.1.3` | `.gitmodules` `external/llvm-project` (not initialized yet) |

Acquisition: `docker build -t ai-sw-engineer .` at the repo root, then
`git submodule update --init --depth 1 <path>` for each needed submodule.

## Finding: no upstream Rust/LLVM path emits 8088-clean code

| Component | Upstream state | Evidence (this run) | Verdict |
|---|---|---|---|
| rustc target | 331 targets in rustc 1.99.1 (`rustc --print target-list`); smallest x86 is **i586** (32-bit). No `i8086`/16-bit target exists. | host command output | Missing; needs custom target + backend (ai_engineer#60) |
| LLVM X86 "16-bit" | `.code16` / `code16` triple is **i386 code with operand-size prefixes**, not 8086 code. Upstream's own test uses triple `i386-unknown-linux-code16`, 32-bit pointers/registers, and even `rdmsr`. Such output faults on an 8088. | `llvm/test/CodeGen/X86/x86-16.ll` (upstream main, fetched) | Unusable for the XT; confirms AGENTS.md's `-m16` ban |
| LLVM MC (assemble/disassemble) | `.code16` assembly parsing exists and is maintained (recent `data32` fixes). Assembly/disassembly of 16-bit encodings is usable infrastructure. | upstream X86 `AsmParser` commits (fetched index) | Reusable inside a new `llvm-i8086` target |

## Reference C toolchains (not Rust; ABI and format precedent)

| Tool | State | License | Use for this issue |
|---|---|---|---|
| gcc-ia16 (tkchia fork of Lambertsen & Jenner IA-16 GCC) | Active; targets MS-DOS by default; `ia16-elf-gcc/as/ld`, libc; far pointers added by the fork; memory models tiny/small/medium/compact/large/huge documented in its man/info pages | GPL (GCC; verify SPDX at pin time) | Reference for 16-bit data layout, memory models, `near`/`far` ABI, ELF-based 16-bit objects |
| Open Watcom v2 | Open-source suite; `wcc` = 16-bit real-mode C compiler, `wlink` links DOS MZ executables, `wasm` assembler | Open Watcom Public License (verify SPDX at pin time) | Independent C oracle for ABI details; `wlink` documents MZ layout; **not** the Rust path |
| NASM | Stable; `BITS 16` emits 16-bit encodings, `-f bin` emits flat binaries (boot sectors), `ndisasm` decodes | BSD-2-Clause (verify SPDX at pin time) | Assemble hand-written crt0/boot bring-up code and decode output; pin at implementation |

Pin at implementation: exact gcc-ia16 commit or PPA release, Open Watcom
release tag, NASM version (`nasm -v`), with acquisition commands in the report.

## Linker / packer position

No stock linker produces XT images from LLVM output: `lld` has no 16-bit
x86 real-mode support, and GNU `ld` knows ELF, not boot-sector/`MZ` fixups.
Precedent in this repo: `llvm-i4004`/`llvm-i8008` pair LLVM MC objects
(ELF32 + machine-private relocations) with a small project-owned
linker/packer (Python). The `llvm-i8086` backend (ai_engineer#60) owns that
call; this issue owns the per-computer side: section layout, `link.ld`,
entry/stack/relocation requirements, and the boot-sector/COM/MZ packaging
commands (`spec/cpu_abi.md`, `platform/link.ld`).

## Independent decoder (acceptance box 6)

Capstone (`CS_ARCH_X86` + `CS_MODE_16`) disassembles 16-bit x86 and is the
planned independent check on backend output, alongside `ndisasm` and the
manufacturer opcode tables. Pin the Capstone version at implementation.

## Execution oracles (acceptance box 7)

| Oracle | State | License / provenance |
|---|---|---|
| Primary: `ai_hw_engineer/system/ibm_pc_xt_5160` RTL platform | Boots original XT BIOS + PC DOS 2.00, runs an assembled 8087 DOS app, `run_all.sh` regressions; 8088EU/CGA/keyboard/floppy/Xebec 10 MB disk | Sibling repo (this project); no CPU emulation or interrupt interception |
| Secondary: `external/martypc` (dbalsom/martypc) | Emulates IBM 5150 and **5160 (XT)**; cycle-accurate 8088 validated against real hardware (Arduino rig); passes SingleStepTests/8088 V2 at 99.9997%; last release 0.4.1, dev 0.5.0 | MIT (per `external/README.md`); submodule **not initialized** in this checkout |

Qualification commands and expected outputs are bring-up work
(`plans/testplan.md`, `bringup/`); this audit only establishes that both
oracles exist and what they claim.

## Missing project-owned work (summary)

1. `llvm-i8086` backend (ai_engineer#60): selection, 8086-only MC, ABI,
   object format, linker/packer, lit/MC/exec tests. **Nothing here duplicates it.**
2. This issue's profile: `rust/i8086-xt-none.json` (provisional draft
   committed), `platform/link.ld`, crt0/HAL contract, image packaging
   commands, smoke programs, regression tests, report.
3. Pinned reference-tool versions (gcc-ia16, Open Watcom, NASM, Capstone)
   once bring-up consumes them.
