# IBM PC XT 5160

CPU: i8086 (Intel 8088). Backend: `llvm-i8086` (ai_engineer#60; does not exist yet).

## History

IBM introduced the PC XT 5160 in **1983** (IBM history; the hardware
authority is IBM's April 1983 5160 Technical Reference — see Sources). The
XT added a 10 MB fixed disk and six more expansion slots to the 5150
architecture, making larger applications practical without diskette
swapping; the optional Intel 8087 accelerated numeric software. This
folder's machine profile is the 256 KiB, CGA, 8087-installed XT that boots
PC DOS 2.00 from the fixed disk, matching
`ai_hw_engineer/system/ibm_pc_xt_5160`.

## Machine summary

- CPU and clock: Intel 8088, nominal 4.772727 MHz, real mode only
- ROM / RAM: 256 KiB RAM (00000h–3FFFFh); option ROMs C8000h+; BASIC/BIOS F6000h–FFFFFh
- I/O: see [platform/io_map.md](platform/io_map.md)
- Memory map: see [platform/memory_map.md](platform/memory_map.md)
- CPU/ABI profile: see [spec/cpu_abi.md](spec/cpu_abi.md)

## Toolchain profile (ai_sw_engineer#59)

This folder owns the per-computer Rust compiler profile — target spec,
startup/linker, image packaging — reusing the shared `llvm-i8086` ISA
backend rather than duplicating it:

- [spec/toolchain_audit.md](spec/toolchain_audit.md) — audit of existing support
- [spec/cpu_abi.md](spec/cpu_abi.md) — machine/CPU, ISA, ABI, data layout
- [rust/i8086-xt-none.json](rust/i8086-xt-none.json) — provisional Rust target (needs the backend)
- [bringup/](bringup/) — hand-written bring-up baseline (not compiler output)
- [report/final_report.md](report/final_report.md) — status, evidence, gaps

Status: profile drafted; compile-and-execute gates blocked on backend
qualification (ai_engineer#60). No title work starts until this profile
qualifies (ai_engineer#84 dependency).

## Simulators

- Primary (ai_hw_engineer): `system/ibm_pc_xt_5160` — RTL platform, boots original XT BIOS + PC DOS 2.00
- Secondary (external/): `external/martypc` — cycle-accurate 8088 PC/XT emulator (submodule not initialized yet)

## Software

| Title | Year | Original authors | Status | Folder |
|---|---|---|---|---|
| — | — | — | blocked: gated by this toolchain profile | — |

## Sources

- IBM, 5160 Technical Reference, April 1983 (via `ai_hw_engineer/system/ibm_pc_xt_5160/spec/spec.md`)
- IBM PC 5150 reference provenance (BIOS/BASIC/DOS hashes): `ai_hw_engineer/system/ibm_pc/spec/reference/README.md`
- MartyPC: <https://github.com/dbalsom/martypc> (README: 5150/5160 support, hw-validated 8088)
