# IBM PC XT 5160 compiler profile

What "Rust for the IBM PC XT 5160" means here: the machine, the instruction set, how Rust maps to it and what an image must look like. Issue: [ai_sw_engineer#59](https://github.com/hevangel/ai_sw_engineer/issues/59).

The XT has the 5150's CPU, the same BIOS interface (INT 10h, 13h, 16h, 1Ah), the same boot path (the BIOS loads sector 1 of drive A at 0000:7C00) and runs the same DOS programs. So this profile **reuses the 5150 profile unchanged** ([../../../ibm_pc/toolchain/spec/profile.md](../../../ibm_pc/toolchain/spec/profile.md)): the shared `llvm-i8086` backend, its target `i8086-unknown-none`, the runtime and linker scripts of `system/ibm_pc/platform`, and `i8086-ld`. It adds no second target. This file states only what differs, and what was checked on the XT.

This replaces the provisional draft of 2026-10-28 (`rust/i8086-xt-none.json`, an XT-only `platform/link.ld`, `bringup/`), which predated the backend and had a data layout that did not match it and the wrong linker (code review on the issue, 2026-10-31).

## Machine

| | |
|---|---|
| Computer | IBM Personal Computer XT 5160 (1983) as the hardware project models it (`ai_hw_engineer/system/ibm_pc_xt_5160`): 256 KiB, CGA, two 5.25-inch diskette drives, BIOS of 1982-11-08 with Cassette BASIC C1.10 at F6000h; memory map [../../platform/memory_map.md](../../platform/memory_map.md), I/O map [../../platform/io_map.md](../../platform/io_map.md) |
| CPU | Intel 8088 at 4.772727 MHz, real mode: as the 5150 |
| Coprocessor | the hardware profile has an 8087 fitted, but **the compiler does not use it**: no `esc`/`wait` x87 instruction is generated and the independent decoder check rejects any (floating point is software, as on the 5150). Using the 8087 would be a backend feature (an x87 code generator), not part of this profile |
| Fixed disk | the Xebec controller and PC DOS 2.00 on the fixed disk are part of the machine, not of the image formats here: a `.COM`/`.EXE` built here can be copied to the fixed disk by DOS like any other file |

## Rust and LLVM representation

Identical to the 5150 profile: target `i8086-unknown-none`, data layout `e-m:e-p:16:16-p1:32:16:16:16-i8:8-i16:16-i32:16-i64:16-f32:16-f64:16-n8:16-a:8-S16` (`llvm-i8086/spec/abi.md` D1), the D3 calling convention, `extern "msp430-interrupt"` handlers (D5), software `f32`/`f64`, no `f128`, far data pointers in address space 1 (D7), far code (D8) and several segments (D9). `tests/test_profile.py` checks that this file states the D1 layout of `abi.md` exactly.

Two equivalent Rust front ends, as in [../../../../llvm-i8086/rust/README.md](../../../../llvm-i8086/rust/README.md): the patched rustc with `i8086-unknown-none` when it is built (`$I8086_RUSTC`), else the msp430 route (rustc builds for `msp430-none-elf`, whose layout equals D1, and the IR is retargeted). The images of the report were built by the msp430 route.

## Memory

The tiny model is unchanged: one 64 KiB segment for code, data and stack, as on the 5150. The XT's 256 KiB matter in two ways:

- a program reaches RAM above its segment through far pointers (D7) or the HAL's `far_read16`/`far_write16`: `smoke/xt_info_rs` writes and reads back every 16 KiB block from 64 KiB to the top of RAM;
- a multi-segment `.EXE` (D9, `link_exe_seg.ld`) can place segments above 64 KiB; DOS 2.00 loads it where free memory is.

## Image formats

The three formats of the 5150 profile, with the same linker scripts and start-up files:

| Format | On the XT |
|---|---|
| Bootable floppy (`link.ld`, `crt0.s`, `i8086-ld --sign --floppy`) | the 160 KiB single-sided format (40 x 8 x 512). The XT's double-sided drive reads it; `crt0.s` loads stage 2 from head 0 with 8 sectors a track, which is that format's layout. A 180/360 KiB boot image would need a `crt0` that steps through 9 sectors and 2 heads (as the PS/2 Model 30 profile's does) |
| DOS `.COM` and `.EXE` (`link_com.ld`, `link_exe_seg.ld`) | run under the original PC DOS 2.00. The DOS disk with a program on it is made with `pcdisk`, which handles the 180 KiB PC DOS 2.00 diskette (and 160, 320, 360, 720 KiB) |

Memory and size checks: the linker refuses region overflows; `build_profile.sh` checks the floppy and `.COM` sizes; `check_images.py` (the 5150 profile's, unchanged apart from reading the floppy size from `images.json`) checks placement, entry, file formats and the decoders.

## Original software

Not in the repository; `scripts/fetch_reference.py` fetches them from the pinned PCjs revisions and checks the SHA-256:

| File | What | SHA-256 |
|---|---|---|
| `xtbios.bin` | IBM 5160 BIOS, 1982-11-08 (8 KiB) | `2798f8a9a7dcab651ede40373d35269ad573b75acf811115c263e034ef80a1f2` |
| `xtbasic.bin` | IBM Cassette BASIC C1.10 (32 KiB) | `3033d1a54c99d7e2aa1fc7c8c2e51a56ae1b61bf4e70e8aa580f43c43e37a63e` |
| `pcdos200.img` | PC DOS 2.00 disk 1, 180 KiB | `934e62ceb797537600847902e627908969bd99df6a58637c4045e1ecaeb53a88` |

Sources: [PCjs](https://github.com/jeffpar/pcjs/tree/1a91d38eb12e5fdf91116b21ad9ef1935710a496/machines/pcx86/ibm/5160/rom) revision `1a91d38e`, [PCjs diskettes](https://github.com/jeffpar/pcjs-diskettes/tree/cd3433748b09837a4c60ca7066f3642772b2d711/pcx86/sys/dos/ibm/2.00) revision `cd343374` (the disk converted from PCjs's sector JSON to flat bytes). IBM and Microsoft copyrights; never committed.

## Execution

The images run on MartyPC's IBM 5160 model ([external/martypc](../../../../external/martypc), pinned `05c0d088`, MIT, unmodified; its author validated the 8088 and bus timing against real hardware), driven by `system/ibm_pc/emulator/tools/martypc_oracle` with `MARTY_MACHINE=5160`. The expected screens do not come from MartyPC: see `scripts/run_independent.sh` and [../report/report.md](../report/report.md).

## Not supported

Booting from the fixed disk, 360 KiB boot images, the 8087 in generated code, more than one 64 KiB segment in the tiny model, the 5150-only cassette port. The RTL of the hardware project was not run for this profile (not reachable from this session); MartyPC is the execution oracle.
