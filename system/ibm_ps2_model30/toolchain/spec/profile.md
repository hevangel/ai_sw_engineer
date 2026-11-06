# IBM PS/2 Model 30 compiler profile

What "Rust for the IBM PS/2 Model 30" means here. Issue: [ai_sw_engineer#60](https://github.com/hevangel/ai_sw_engineer/issues/60). The Model 30 has an 8086 and the PC BIOS interface, so the profile is the 5150 profile ([../../../ibm_pc/toolchain/spec/profile.md](../../../ibm_pc/toolchain/spec/profile.md)) with a different boot medium: the shared `llvm-i8086` backend and target `i8086-unknown-none`, the shared HAL (`hal.s`, `ibm_pc_hal`), the shared DOS start-up and linker scripts, and `i8086-ld`. This file states what differs.

## Machine and CPU

| | |
|---|---|
| Computer | IBM Personal System/2 Model 30, type 8530-002/021: system board with 640 KiB, MCGA, one 3.5-inch 720 KiB diskette drive, BIOS revision 0 of 09/02/86 (64 KiB at F0000h, model byte FAh); maps in [../../platform/memory_map.md](../../platform/memory_map.md) and [../../platform/io_map.md](../../platform/io_map.md) |
| CPU | Intel 8086 at 8 MHz, real mode. The instruction set is the 8088's (the 8086 has a 16-bit bus and a six-byte queue, which change timing, not instructions): the same 8086/8088 set the backend generates and the decoder check enforces; no 80186+ instruction, no x87 (the Model 30's 8087 socket is optional; the compiler does not use it) |
| Data layout, ABI | identical to the 5150 profile: `e-m:e-p:16:16-p1:32:16:16:16-i8:8-i16:16-i32:16-i64:16-f32:16-f64:16-n8:16-a:8-S16` (`llvm-i8086/spec/abi.md` D1), the D3 calling convention, interrupt handlers (D5), software floating point (D6), far pointers and segments (D7-D9) |
| Rust front end | the msp430 route or the patched rustc, as for the 5150 ([../../../../llvm-i8086/rust/README.md](../../../../llvm-i8086/rust/README.md)); the images of the report were built by the msp430 route |

## Image formats

| Format | Files | On the Model 30 |
|---|---|---|
| Bootable 720 KiB diskette | [../../platform/link.ld](../../platform/link.ld) (the 5150 layout), [../../platform/runtime/crt0.s](../../platform/runtime/crt0.s), `i8086-ld --sign --floppy x.img --floppy-kib 720` | the BIOS loads sector 1 at 0000:7C00; `crt0.s` reads the rest of the image in the disk's logical order (9 sectors, then head 1, then the next cylinder) from the boot drive, clears `.bss` and calls `main`. The image file is the binary in logical sector order, padded to 737,280 bytes |
| DOS `.COM`, `.EXE` | the 5150 platform's `link_com.ld`, `link_exe_seg.ld`, `crt0_com.s`, `dos.s` | run under PC DOS 3.30 from the 720 KiB boot disk that `scripts/fetch_reference.py` makes; `pcdisk` adds the program (it handles 720 KiB DOS disks) |

Additions to the HAL: [../../platform/runtime/ps2.s](../../platform/runtime/ps2.s), `ps2_display_combination()` (INT 10h AX=1A00h). Size checks: the linker refuses a region overflow; `build_profile.sh` checks that the diskettes are 720 KiB and the `.COM` fits a segment; `check_images.py` (the 5150 profile's) checks the decoders, relocations, placement, entry and formats, with the diskette size from `images.json`.

## Original software

Not in the repository; `scripts/fetch_reference.py` fetches and checks:

| File | What | SHA-256 |
|---|---|---|
| `68X1687.BIN`, `68X1627.BIN` | BIOS revision 0 (09/02/86), even and odd chips, from the [86Box ROM set](https://github.com/86Box/roms/tree/c4cd8da9034a8557896ff6a975ae58499a83bb80/machines/ibmps2_m30) at commit `c4cd8da9` | `ef2105f6...`, `6651af5f...` |
| `ps2bios.bin` | the 64 KiB BIOS: 68X1687 on the even bytes, 68X1627 on the odd (the order that puts the date `09/02/86` at F000:FFF5 and the model byte FAh at F000:FFFE) | `e3de87dd253468fc3f739f6d07f4444f58bc1b34e1ebfc088d125d5db048c6f7` |
| `pcdos330.img` | PC DOS 3.30 disk 1, 360 KiB, from [pcjs-diskettes](https://github.com/jeffpar/pcjs-diskettes/tree/cd3433748b09837a4c60ca7066f3642772b2d711/pcx86/sys/dos/ibm/3.30) at `cd343374` | `f109f149d9a770f9ac3ab64e0fb03588f54fc5e8dada29722f3eaa9dc8e5af33` |
| `pcdos330_720k.img` | made from it: the 3.30 boot record with the 720 KiB BIOS parameter block, then IBMBIO.COM, IBMDOS.COM and COMMAND.COM in that order (IBMBIO.COM at cluster 2, where the boot record looks for it). Their attribute bytes are 0, not hidden/system as FORMAT /S leaves them (P-A3) | `ec8965c66541e2b981fcb65c57f9d9322d0c94c5bf48b169eee207e9c7fc3404` |

IBM and Microsoft copyrights; never committed.

## Execution oracle

The profile needs an existing emulator of the 8086 Model 30. MartyPC does not model it; MAME 0.264 has the Model 30-286 and later PS/2s but not the 8086 Model 30. 86Box does (`[8086] IBM PS/2 model 30`, internal name `ibmps2_m30`, with MCGA and this BIOS), and its automation socket (`-D AUTOMATION=ON`: `text`, `key`, `wait text`, `fdd load`, `hold`/`step`) can drive it headless. Building it from a pinned commit is the remaining step of the execution gate; see [../report/report.md](../report/report.md).

## Assumption ledger

| ID | Assumption | Why | How it is validated |
|---|---|---|---|
| P-A1 | The RAM and video layout of the memory map are those of the Technical Reference | not checked against hardware | `ps2_info_rs` prints the BIOS's RAM size and tests every 16 KiB block (on the emulator, once it runs) |
| P-A2 | `irq_rs`'s 5150-style keyboard acknowledge (port 61h bit 7) is harmless on the Model 30 | the Model 30's keyboard interface is not the 5150's PPI | `irq_rs` on the emulator: the scan codes typed must be printed |
| P-A3 | PC DOS 3.30 boots from the 720 KiB disk made by `fetch_reference.py` with IBMBIO.COM and IBMDOS.COM not marked hidden/system | the 3.30 boot record finds IBMBIO.COM by the first directory entry and cluster 2, not by attribute | booting it on the emulator |

## Not supported

The 8087 in generated code; the 5.25-inch external drive; MCGA graphics modes from Rust (the BIOS teletype is the console); more than one 64 KiB segment in the tiny model; the RTL of the hardware project (not reachable here).
