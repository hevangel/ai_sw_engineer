# Upstream compiler support audit — Intel 80286 (i286)

Audited 2026-10-25 for hevangel/ai_sw_engineer#66 (stage 1). Sources are
linked inline; version claims are as of that date.

## Summary

No upstream LLVM/rustc support for the 80286 exists, and no maintained LLVM
fork provides one. The practical existing compiler for 286-class CPUs is the
gcc-ia16 port (plus Open Watcom). A project-owned LLVM backend is therefore
required, exactly as for the 4004 and 8008.

## LLVM (upstream, pinned llvmorg-23.1.3 in this repo)

- The X86 backend's minimum target is the **80386**, optionally in `code16`
  16-bit mode (`i386-unknown-none-code16` is a known custom-target basis).
  16-bit code generation dates to 2014 work on the `code16` mode
  ([Phoronix report](https://www.phoronix.com/news/MTU3MTM)).
- Even i386 support has correctness caveats: upstream tracked that i386
  builds could emit `bswap`/`cmpxchg`/`xadd`, which the 386 does not have
  ([LLVM issue #58470](https://github.com/llvm/llvm-project/issues/58470)).
  The 286 subset is far narrower still (no 32-bit operands, no `movzx`/
  `movsx`/`setcc`/`shld`/`shrd`/`bt`, no FS/GS, no 386 protected-mode
  paging), so a code16-mode i386 backend cannot emit legal 286 code without a
  new instruction-selection layer and instruction legality model.
- Search evidence: no maintained out-of-tree LLVM i286/80286 backend fork was
  found (general and targeted searches, 2026-10-25). Nearby out-of-tree
  vintage-x86 backends exist for the 8080/8085
  ([llvm-8085](https://github.com/apullin/llvm-8085)) but not the 286.
- License if reused: Apache-2.0 WITH LLVM-exception (same as the pinned tree).

## GCC — gcc-ia16 (the main existing 286 toolchain)

- [gcc-ia16](https://github.com/tkchia/gcc-ia16): GCC 6.3.0-based port with an
  `ia16` backend targeting 8086/80186/80286; `-march=i286` enables 286 code,
  including **partial protected-mode code generation**
  ([IA-16 options](https://wonderful.asie.pl/doc/gcc-ia16/gcc-6.3.0/gcc/IA_002d16-Options.html)).
- Companion [binutils-ia16](https://github.com/tkchia/binutils-ia16) (ld with
  DOS EXE/COM output); build guide and prebuilt Ubuntu packages
  ([toolchain guide](https://flaterco.com/kb/gcc-i16.html),
  [PPA](https://launchpad.net/~tkchia/+archive/ubuntu/build-ia16)).
  GPL-licensed. This is the reference toolchain ELKS also builds against.
- Relevance to this project: a C toolchain, not a Rust target — useful as an
  oracle for ABIs/relocations and as a cross-check disassembly source, not as
  the delivery vehicle (the project compiles Rust).

## Rust

- No tiered 16-bit x86 target exists ([platform support](https://doc.rust-lang.org/rustc/platform-support.html)).
  The community recipe for 8086-class Rust is a custom target JSON with
  `llvm-target: "i386-unknown-none-code16"` plus `-Zbuild-std=core`
  ([Reddit thread](https://www.reddit.com/r/rust/comments/zzqfn8/how_can_i_compile_rust_for_16bit_x86_intel_8086)) —
  which emits 386 instructions in 16-bit encoding and therefore **does not
  run on an 80286**. A native i286 LLVM backend plus a project target JSON
  (`i286-none`) is required for exact-ISA Rust.

## Open Watcom

- Open Watcom C/C++ (SaaS-permissioned open source) has long-standing
  16-bit x86 code generation with 286 codegen support; historically the other
  mainstream DOS/286 compiler. Same role as gcc-ia16: reference/oracle, not
  the Rust path.

## Consequence for the profile (issue #66)

Per the issue's readiness policy, the compiler configuration is blocked until
the project-owned backend is implemented and qualified (roadmap coordination:
hevangel/ai_engineer#63). Requirements research and the machine/compiler
contract proceed in parallel ([machine_profile.md](machine_profile.md)).
