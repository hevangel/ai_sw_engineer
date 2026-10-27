# llvm-m6800

| | |
|---|---|
| Chip | Motorola MC6800 (M6800 Microcomputer System, announced March 1974) — MEK6800D2, MIKBUG/SWTBUG monitors, SWTPC 6800, early embedded designs; architecture carried forward by 6801/6301/68HC11 |
| Word / address width | 8-bit data, 16-bit addresses (flat 64 KiB), **big-endian** |
| Strategy | new project-owned LLVM target on the pinned `external/llvm-project` — **no LLVM, GCC or Rust support exists anywhere** (see [spec/audit_upstream.md](spec/audit_upstream.md)); a 6809 or 68k target is explicitly not interchangeable (issue #61) |
| Rust target | planned `rust/m6800-none.json` (does not exist yet) |
| Status | **audit and specification stage (2026-10-26, issue ai_engineer#61)** — no backend code, no target JSON, no working compiler is claimed. The executor side of the future qualification chain is validated: the ai_hw_engineer MC6800 RTL passes its full-ISA vector replay fresh this session. |

This folder holds the audit, the compiler contract and the implementation
plan only. Like `llvm-i4004`/`llvm-i8008`, it will hold the backend overlay,
target JSON, runtime, assembler/linker and tests as milestones land.

## Independent executor (validated)

`$AI_HW_ENGINEER/design/motorola_6800` — full documented ISA (197 codes) on a
functional bus, verified against a PRM-derived Python ISS (1,951/1,951
instruction-boundary checks) and 50 vendored third-party self-checking
programs ([hakanrw/mc6800](https://github.com/hakanrw/mc6800)); formal
BMC/prove/cover. Fresh run 2026-10-26: **full-ISA simulation PASSED**.

## Layout

| Path | Contents |
|---|---|
| [spec/audit_upstream.md](spec/audit_upstream.md) | audit of existing support (LLVM, GCC, assemblers, emulators, Rust), versions and licenses |
| [spec/compiler_profile.md](spec/compiler_profile.md) | MC6800 compiler contract: ISA subset, ABI, data layout, runtime, memory map, executable format |
| [plans/implementation_plan.md](plans/implementation_plan.md) | staged backend plan (spec freeze → MC → CodeGen → runtime/linker → Rust profile) |
| [report/audit_2026-10-26.md](report/audit_2026-10-26.md) | dated audit record with the fresh executor validation and explicit gaps |
