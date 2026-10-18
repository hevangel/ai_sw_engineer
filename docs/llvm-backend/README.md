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
