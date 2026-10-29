# Systems

One folder per historical machine. Each holds the machine's `platform/` (memory map, I/O, boot, linker script, runtime/HAL), a host Rust `emulator/` for complete-machine execution, and one subfolder per software title reimplemented in Rust. Working emulators: [Busicom 141-PF](busicom_141pf/emulator/README.md), the [IBM PC 5150](ibm_pc/emulator/README.md) (it boots the original BIOS and PC DOS 1.00) and the [Atari 400/800 first slice](atari_400_800/emulator/README.md); emulators for the other listed systems remain planned.

| System | Year | CPU | LLVM backend | HW simulator in ai_hw_engineer | Software titles | Folder |
|---|---|---|---|---|---|---|
| Busicom 141-PF calculator | 1971 | Intel 4004 | `llvm-i4004` | `system/busicom_141pf` (4004/4001/4002/4003 RTL) | [firmware](busicom_141pf/firmware/) (M5: 4,095 B ROM, 42/42 on RTL and chips) | [busicom_141pf/](busicom_141pf/) |
| Micral N | 1973 | Intel 8008 | `llvm-i8008` | `system/micral_n` (8008 RTL; our Micral N harness in `platform/sim`) | [boot](micral_n/boot/) (241 B), [monitor](micral_n/monitor/) (2 KiB ROM, 7/7), [shooting_stars](micral_n/shooting_stars/) (4/4), [basic](micral_n/basic/) (SCELBAL, 10/10) | [micral_n/](micral_n/) |
| Apple II | 1977 | MOS 6502 | `llvm-mos6502` | `system/apple_ii` (in progress), `design/mos_6502` | — | planned |
| Atari 400/800 | 1979 | MOS 6502 (Atari "SALLY" 6502C) | `llvm-mos6502` | `design/mos_6502` (CPU only; whole machine planned, [ai_engineer#355](https://github.com/hevangel/ai_engineer/issues/355)) | — | [atari_400_800/](atari_400_800/) (emulator first slice: CPU passes Klaus Dormann via its bus; no video/audio/DMA yet) |
| Atari 2600 (VCS) | 1977 | MOS 6507 | `llvm-mos6502` | `design/mos_6502` (CPU only) | — | planned |
| IBM PC 5150 | 1981 | Intel 8088 | `llvm-i8086` | `system/ibm_pc`, `system/ibm_pc_xt_5160` | [CHKDSK](ibm_pc/chkdsk/README.md) (Rust, identical to the original on 24 test disks) | [ibm_pc/](ibm_pc/) (emulator boots the original BIOS and PC DOS 1.00 and matches the RTL; `llvm-i8086` has the assembler, disassembler, linker, a rustc `i8086-unknown-none` target and Rust programs; compiler profile in [ibm_pc/toolchain/](ibm_pc/toolchain/README.md)) |
| IBM PC XT 5160 | 1983 | Intel 8088 | `llvm-i8086` | `system/ibm_pc_xt_5160` (boots PC DOS 2.00) | — | [ibm_pc_xt_5160/](ibm_pc_xt_5160/) (toolchain profile drafted, ai_sw_engineer#59; shares the `llvm-i8086` backend, now available) |
| Commodore 64 | 1982 | MOS 6510 | `llvm-mos6502` | `design/mos_6502` (CPU only) | — | planned |
| Apple IIe | 1983 | MOS 6502 (65C02 from 1985 Enhanced IIe) | `llvm-mos6502` | `design/apple_iie_mmu`, `apple_iie_iou`, `apple_iie_pal` | — | planned |
| Nintendo Entertainment System | 1983 (Famicom) / 1985 (NES) | Ricoh 2A03 (6502 core, no decimal mode) | `llvm-mos6502` | `design/mos_6502` (CPU only) | — | planned |

Year sources: dates follow the [Computer History Museum timeline](https://www.computerhistory.org/timeline/computers/) and the system overview each `system/<name>/README.md` cites; month-level claims are made only in those READMEs with their sources. Where `ai_hw_engineer` has only a CPU core, the whole-machine oracle comes from a third-party emulator in `external/` until the machine is built there.

## Layout

```text
system/<system>/
├── README.md          — machine history, sources, CPU, memory, software list
├── emulator/          — host Rust system library, executable, spec and replay tests
├── platform/          — shared by every title on this machine
│   ├── README.md
│   ├── memory_map.md  — ROM/RAM/I/O address map and budgets
│   ├── io_map.md      — ports, soft switches, peripheral behavior
│   ├── link.ld        — linker script for the target image
│   └── runtime/       — crt0/reset vector, HAL implementation for the real machine
└── <software>/        — see _template/software/ and spec/repository.md
```

Create with `./scripts/new_system.sh <system> <chip>` and `./scripts/new_software.sh <system> <software>`.
