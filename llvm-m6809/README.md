# llvm-m6809

| | |
|---|---|
| Chip | Motorola MC6809 (1978) — the most advanced 8-bit Motorola CPU: TRS-80 Color Computer, Dragon 32/64, Vectrex, SWTPC 6809; ran OS-9; shaped the Hitachi 6309 and 68HC11 |
| Word / address width | 8-bit data, 16-bit addresses (flat 64 KiB), **big-endian** |
| Strategy | new project-owned LLVM target on the pinned `external/llvm-project` — the only LLVM 6809 attempt (SourceForge `llvm6809be`, LLVM 2.x era) is abandoned and never landed; GCC's gcc6809 and CMOC are C-only references (see [spec/audit_upstream.md](spec/audit_upstream.md)) |
| Rust target | planned `rust/m6809-none.json` (does not exist yet) |
| Status | **audit and specification stage (2026-10-26, issue ai_engineer#62)** — no backend code, no target JSON, no working compiler is claimed. The executor side of the future qualification chain is validated: the ai_hw_engineer MC6809 RTL passes its third-party functional and interrupt suites fresh this session. |

## Independent executor (validated)

`$AI_HW_ENGINEER/design/motorola_6809` — full documented ISA (pages 1/2/3)
verified against the hardware-validated
[reyco2000/6809_6309_code_tester](https://github.com/reyco2000/6809_6309_code_tester)
self-checking suites plus formal BMC/prove/cover. Fresh run 2026-10-26:
**6809_functional_test and 6809_interrupt_test both PASSED** (Verilator,
DEV_EXIT=0).

## Layout

| Path | Contents |
|---|---|
| [spec/audit_upstream.md](spec/audit_upstream.md) | audit of existing support (LLVM, GCC/CMOC, LWTOOLS, assemblers, emulators, Rust), versions and licenses |
| [spec/compiler_profile.md](spec/compiler_profile.md) | MC6809 compiler contract: ISA subset, indexed addressing, ABI, data layout, runtime, executable formats |
| [plans/implementation_plan.md](plans/implementation_plan.md) | staged backend plan |
| [report/audit_2026-10-26.md](report/audit_2026-10-26.md) | dated audit record with the fresh executor validation and explicit gaps |
