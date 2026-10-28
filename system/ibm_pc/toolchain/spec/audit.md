# Audit of existing compiler support, and pinned versions

Done first, as the issue asks (2026-10-27). Each finding has the command that shows it, run in the `ai-sw-engineer` image.

## What upstream provides for the 8086 (nothing usable)

| Component | Finding | Evidence |
|---|---|---|
| LLVM 23.1.3 (`external/llvm-project`, `0d261d1ca552`) | no 8086 target. Targets in `llvm/lib/Target`: AArch64 AMDGPU ARC ARM AVR BPF CSKY DirectX Hexagon Lanai LoongArch M68k Mips MSP430 NVPTX PowerPC RISCV SPIRV Sparc SystemZ VE WebAssembly X86 XCore Xtensa. X86 has a 16-bit *mode* for 80386 and later, not an 8086 | `ls external/llvm-project/llvm/lib/Target` |
| clang `-m16` (clang 20 in the image) | produces 80386 code with operand-size prefixes: `66 55` is `push bp` as `pushw` with a 66h prefix, which an 8086 does not decode | `clang --target=i386-none-elf -m16 -c a.c && llvm-objdump -d a.o` |
| rustc nightly-2026-10-15 (`7df81381d`) | 334 built-in targets, none for the 8086 or any 16-bit x86; the x86 ones are `i586`/`i686`/`i386` (32-bit) | `rustc +nightly-2026-10-15 --print target-list \| grep -iE '8086\|i286\|i386\|i586\|i686'` |
| GNU as/objdump 2.38 (binutils) | decodes and encodes the 8086 (`-m i8086`); used here as an independent decoder, not as a compiler | `objdump -D -b binary -m i8086` |

Other compilers exist outside LLVM and rustc (GCC forks for 16-bit x86, Open Watcom, Turbo C, bcc); none of them compiles Rust, and none is part of this profile. They were not built or tested for this audit.

## What the project owns

| Component | Where | Notes |
|---|---|---|
| LLVM backend `I8086` | [llvm-i8086/target](../../../../llvm-i8086/target) (overlay applied to a copy of the pinned LLVM) | Apache-2.0 WITH LLVM-exception |
| Assembler, disassembler, object writer | same backend (`llvm-mc`, `llvm-objdump`) | verified against 8,370 instruction instances from a physical-chip corpus |
| Linker and image packager `i8086-ld` | [llvm-i8086/asm](../../../../llvm-i8086/asm) | MIT |
| Runtime library | [llvm-i8086/rt](../../../../llvm-i8086/rt) (multiply, divide, shifts, `memcpy`, ... and `compiler_builtins`) | MIT; `compiler_builtins` is MIT/Apache-2.0 from the rust-lang/rust tree |
| rustc with `i8086-unknown-none` | [llvm-i8086/rust/patches](../../../../llvm-i8086/rust/patches/apply_i8086_target.py): a patch to rust-lang/rust `7df81381d` | MIT/Apache-2.0; built against our LLVM |
| Platform runtime, linker scripts, HAL | [../../platform](../../platform) | this issue's profile |

## Pinned versions

`scripts/versions.sh` prints these from the tools themselves; the table is its output as checked in on 2026-10-27.

| Item | Version |
|---|---|
| Docker image | `ai-sw-engineer:latest` built from the repository `Dockerfile`; base `mrkits/rust-mos:13f2838f9-334fc98-8f3a80f8` |
| Host Rust (linker, emulator, tools) | rustc/cargo 1.99.1 (`58cf8f9c3`) |
| Compiler Rust | rust-lang/rust `7df81381d027710708209f36b98d11b1c88f8d2e` (nightly-2026-10-15, rustc 1.101.0-nightly) + `apply_i8086_target.py` |
| LLVM | `external/llvm-project` `0d261d1ca552c95a8f007e061c787ac7132fbcbc` (llvmorg-23.1.3) + the `llvm-i8086/target` overlay |
| C++ build tools | gcc 11.4.0, cmake 3.22.1, ninja 1.10.1 |
| Independent decoders | GNU binutils objdump 2.38; Capstone 5.0.x (Python) |
| Independent machine models | the hardware project's RTL (`ai_hw_engineer`, Verilator), MartyPC (`external/martypc` `05c0d08`), the Rust emulator in this repository |

## Reproducible acquisition and build

```bash
git submodule update --init --depth 1 external/llvm-project external/martypc
docker build -t ai-sw-engineer .
D="docker run --rm -v $PWD:/workspace -v ai-sw-llvm-build:/build -w /workspace ai-sw-engineer:latest"
$D llvm-i8086/scripts/build_llvm.sh             # llc, llvm-mc, llvm-objdump with the I8086 target
$D llvm-i8086/scripts/build_llvm_for_rust.sh    # LLVM libraries for rustc
$D llvm-i8086/scripts/rust_build.sh             # fetches rust-lang/rust 7df81381d, patches it, builds stage 1
$D llvm-i8086/scripts/build_rt.sh               # runtime library
```

About 10 minutes for LLVM and 15 for rustc and its libraries on 32 cores; everything lands in the volume `ai-sw-llvm-build` and in `llvm-i8086/install`.
