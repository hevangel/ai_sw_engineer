# Running the programs on a real IBM PC 5150

Everything in this repository was checked against the original BIOS and PC DOS 1.00 running on the
hardware project's RTL (`ai_hw_engineer`), which is an independent model of the chips, and against the
Rust emulator. A run on a physical 5150 needs the machine, a 5.25-inch drive and a way to write a
160 KiB disk image to a real floppy; none of these is reachable from the development environment
(checked 2026-10-27: the host has COM1 and USB mass-storage devices, no floppy controller or flux
reader). The programs and what a correct run shows are ready, so the run is one step for whoever has the machine.

## Disks

- **Boot floppy** `hello_rs`: `system/ibm_pc/emulator/scripts/rtl_boot_images.sh` builds
  `oracle/boot/images/hello_rs.img` (the `.img` files are build products, `.gitignore`d; their SHA-256 is
  line 3 of `oracle/boot/hello_rs.txt`). 160 KiB, single-sided, 8 sectors per track, 40 tracks.
  Insert as drive A and boot. Expected screen, from the RTL: `oracle/boot/hello_rs.txt`
  (`Rust on the 8088`, `fib(20) = 6765`, `12! = 479001600`, ...).
- **DOS programs**: put `HELLO.COM`, `HELLOX.EXE`, `SEGX.EXE` (the two-segment `.EXE`) and the Rust
  `CHKDSK.COM` (`system/ibm_pc/chkdsk/bin/chkdsk.com`) on a PC DOS 1.00 disk with
  `system/ibm_pc/platform/tools/pcdisk` (`pcdisk add <image> <name> <file>`) and run them from the `A>`
  prompt. Expected screens: `system/ibm_pc/emulator/oracle/dos/*.txt`; for CHKDSK the original's output on
  the same disk (`system/ibm_pc/chkdsk/oracle/`).

## Writing the image

Any 5.25-inch 160 KiB-capable setup: a 360 KB drive on a PC (`dd`/RawWrite style raw write with the
drive in 160 KiB mode, or ImageDisk), or a flux writer (Greaseweazle: `gw write --format=ibm.160 hello_rs.img`).

## Recording the result

Photograph or transcribe the screen and compare it with the transcript line by line. A difference is a
bug in the RTL model, the emulator or the toolchain, and is the most valuable finding this project can get.
