# IBM PC XT 5160

CPU: i8086 (Intel 8088). Backend: `llvm-i8086` (shared with the 5150).

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

- [toolchain/](toolchain/README.md) — the profile: spec, audit, smoke program, build and run scripts, report
- historical (2026-10-28 draft, superseded): `spec/toolchain_audit.md`, `spec/cpu_abi.md`, `bringup/`, `report/final_report.md`

Status (2026-11-06): the profile reuses the 5150's toolchain; six Rust/IR images build
reproducibly, pass the independent decoder and relocation checks, and run on MartyPC's 5160
with the original XT BIOS and PC DOS 2.00, showing the expected screens
([report](toolchain/report/report.md)). Not run on the hardware project's XT RTL or real hardware.

## Simulators

- Primary (ai_hw_engineer): `system/ibm_pc_xt_5160` — RTL platform, boots original XT BIOS + PC DOS 2.00
- Secondary (external/): `external/martypc` — cycle-accurate 8088 PC/XT emulator, the profile's execution oracle (`MARTY_MACHINE=5160`)

## Software

Catalog of candidates: [software_catalog.md](software_catalog.md) (ai_engineer#84).

| Title | Year | Original authors | Status | Folder |
|---|---|---|---|---|
| SORT.EXE (PC DOS 2.00) | 1983 | Microsoft for IBM | reimplemented in Rust; original and rewrite identical on MartyPC's 5160 and the Rust emulator, 16 cases | [sort/](sort/README.md) |
| FIND.EXE (PC DOS 2.00) | 1983 | Microsoft for IBM | reimplemented in Rust; identical on both emulators, 22 cases | [find/](find/README.md) |
| COMP.COM (PC DOS 2.00) | 1983 | IBM | reimplemented in Rust; identical on both emulators, 10 cases | [comp/](comp/README.md) |
| TREE.COM (PC DOS 2.00) | 1983 | IBM (David Litton) | reimplemented in Rust; identical on both emulators, 9 cases (drive B disks with subdirectories) | [tree/](tree/README.md) |

## Sources

- IBM, 5160 Technical Reference, April 1983 (via `ai_hw_engineer/system/ibm_pc_xt_5160/spec/spec.md`)
- IBM PC 5150 reference provenance (BIOS/BASIC/DOS hashes): `ai_hw_engineer/system/ibm_pc/spec/reference/README.md`
- MartyPC: <https://github.com/dbalsom/martypc> (README: 5150/5160 support, hw-validated 8088)
