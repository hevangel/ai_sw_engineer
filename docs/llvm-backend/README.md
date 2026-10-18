# Writing a new LLVM backend (4004, 8008, real-mode 8086)

Order of work (mirrors `llvm-_template/plans/implementation_plan.md`):

1. `spec/isa.md`, `spec/abi.md`: data layout, register classes, calling convention, relocations.
2. Register the target: `Triple` arch enum, ELF `e_machine` (use an unassigned/experimental value and record it), CMake `LLVM_EXPERIMENTAL_TARGETS_TO_BUILD`.
3. TableGen: `<Chip>RegisterInfo.td`, `<Chip>InstrInfo.td` (encodings first), `<Chip>.td`.
4. MC layer: `MCTargetDesc`, `AsmParser`, `InstPrinter`, `MCCodeEmitter`, ELF object writer + fixups → `llvm-mc` works.
5. CodeGen: `ISelLowering` (legalize wider types into the native width), `ISelDAGToDAG`, `FrameLowering`, `InstrInfo` copy/spill, `AsmPrinter` → `llc` works.
6. `lld` support for the relocations, or a small standalone linker/packer in `asm/`.
7. rustc: add the `arch` to a rustc fork's `rustc_target` + link it to the built LLVM (follow `external/rust-mos`), then `-Zbuild-std=core`.

Chip-specific hazards to settle in `abi.md` before code:

- **4004**: 4-bit data, 12-bit ROM addresses, 3-level hardware stack (no RAM stack — calls must be inlined or use a software stack in 4002 RAM), RAM reached only through SRC/RDM/WRM, 16 4-bit index registers. Expect a "source-level" CPU: most of Rust's model (byte-addressable memory, pointers) has to be emulated.
- **8008**: 14-bit address, 7-level internal stack, no stack pointer in memory, memory via HL only.
- **8086 real mode**: segmented 20-bit addresses; upstream x86 `-m16` emits 386+ instructions (operand-size prefixes, `movzx`) — unusable for an 8088.

References: [LLVM Writing an LLVM Backend](https://llvm.org/docs/WritingAnLLVMBackend.html), [Writing an LLVM Target (Code Generator)](https://llvm.org/docs/CodeGenerator.html), llvm-mos (`external/llvm-mos/llvm/lib/Target/MOS/`) as a worked small-CPU backend.

## Status / lessons from llvm-i4004 (MC layer, LLVM 23.1.3)

The first new backend, `llvm-i4004`, has its MC layer complete (milestone 2):
`llvm-mc -triple=i4004 -show-encoding` assembles all 46 instructions; the ELF
writer stamps `e_machine 0x4004`. Reusable findings for the next backends:

- **Overlay, never edit the submodule.** Target sources live in
  `llvm-<chip>/target/lib/Target/<Chip>/` and a `scripts/apply_overlay.py`
  copies them onto a scratch LLVM tree and makes small idempotent, anchored
  edits to `Triple.h/.cpp`, `ELF.h`, `ELF.cpp`, `ELFObjectFile.h` and
  `llvm-readobj/ELFDumper.cpp`. The private `e_machine` and `R_<CHIP>_*`
  relocations are added via an `ELFRelocs/<Chip>.def`.
- **Build in a Docker named volume**, not the Windows bind mount: put the LLVM
  source copy and build dir in e.g. `-v ai-sw-llvm-build:/build`. The one-time
  source copy from the bind mount is the slow part; cache it in the volume.
- **Use host `gcc`/`g++`** to build LLVM: the `ai-sw-engineer` image`s
  `/usr/local/bin/clang` defaults to the `mos` bare-metal target and fails the
  CMake host-compiler link check.
- **Strip CR from the LLVM shell helper scripts** (`config.guess`,
  `config.sub`, `*.sh`) in the scratch copy: Windows git checks the submodule
  out with CRLF and `/bin/sh` chokes on the CR.
- **LLVM 23.1 `-gen-instr-info`** requires every target to remap the standard
  `ptr_rc` pseudos onto a concrete register class with
  `defm : RemapAllTargetPseudoPointerOperands<SomeRegClass>;` -- even for an
  MC-only target. An MC-only target also still needs a minimal `LLVM<Chip>CodeGen`
  component library (a stub `LLVMInitialize<Chip>Target` is enough) so tools
  that link `all-targets` resolve the component.
