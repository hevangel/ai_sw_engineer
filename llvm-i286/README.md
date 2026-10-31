# llvm-i286

| | |
|---|---|
| Chip | Intel 80286 — introduced February 1982; the CPU of the IBM PC AT 5170 (1984) |
| Word / address width | 16-bit data, 24-bit physical addresses (16-bit segmented); real mode (compatibility) and protected mode |
| Strategy | new experimental LLVM target on the pinned `external/llvm-project` — no usable backend exists upstream or in any maintained fork (see [spec/audit_upstream.md](spec/audit_upstream.md)) |
| Rust target | planned `rust/i286-none.json` (does not exist yet) |
| Status | **audit and specification stage (2026-10-25, issue #66 stage 1)** — no backend code, no target JSON, no working compiler is claimed. The per-computer configuration (hevangel/ai_sw_engineer#66) is blocked by qualification of this backend, coordinated on the roadmap as hevangel/ai_engineer#63. |

This folder currently contains the audit, the machine/compiler contract and
the implementation plan only. Like `llvm-i4004` and `llvm-i8008`, it will hold
the backend overlay, target JSON, runtime, assembler/linker and tests as the
milestones land.

## Layout

| Path | Contents |
|---|---|
| [spec/audit_upstream.md](spec/audit_upstream.md) | audit of existing compiler support (LLVM, GCC ia16, Rust, Watcom), versions and licenses |
| [spec/machine_profile.md](spec/machine_profile.md) | IBM PC AT 5170 compiler contract: CPU modes, entry, memory map, executable formats, ABI, instruction policy |
| [plans/implementation_plan.md](plans/implementation_plan.md) | staged backend plan (ISA/MC → CodeGen → runtime/linker → per-machine profiles) |
| [report/audit_2026-10-25.md](report/audit_2026-10-25.md) | dated audit record with explicit gaps |

## Simulators (planned executors for generated images)

- ai_hw_engineer 80286/PC AT reconstruction (canonical when it exists).
- Existing qualified 8086/286 emulators in other languages (e.g. 86Box/PCem-class
  machine emulators) for bring-up, per the issue's readiness policy.
