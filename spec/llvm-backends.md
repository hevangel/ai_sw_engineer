# LLVM backends

Read this before creating or changing anything under `llvm-<chip>/`.

- Prefer existing backends: llvm-mos for the 6502 family. Only write a new backend where none exists (4004, 8008, real-mode 8086 — LLVM's x86 `-m16` emits 386+ opcodes and is not acceptable for an 8088).
- Each backend records its data layout and calling convention in `spec/abi.md` before code is written; the Rust target JSON must match it exactly.
- Every instruction selection pattern gets a lit/FileCheck test; every MC encoding gets an encode test checked against the manufacturer opcode table.
- Once a backend compiles and links a real program, `spec/codegen.md` (design doc) and `spec/abi.md` describe the backend as it is, not as it was planned. A change to lowering or pseudo expansion, the calling convention or saved registers, frames, call strategy, the linker/packer, or a runtime routine's contract updates them in the same change. A superseded design is marked frozen or historical with a date, not deleted. If the two docs disagree with the code, the docs are the defect.

Related: [documentation.md](documentation.md) (design-doc rules), [escaped-defects.md](escaped-defects.md) (whole-program size gate), [verification.md](verification.md).
