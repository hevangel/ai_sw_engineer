/* IBM PC XT 5160 target image linker script — from platform/memory_map.md.
 *
 * Near-only real-mode profile (spec/cpu_abi.md). The backend linker/packer
 * (owned by ai_engineer#60) selects ONE image kind per link and enforces its
 * size cap; this script only names the regions and section order. Final
 * syntax follows that linker (ld-like here, mirroring llvm-i8008); the
 * packer, not this file, appends the AA55 signature or the MZ header.
 */

MEMORY {
  /* Boot sector: BIOS loads 512 B to 0000:7C00, last two bytes 55 AA. */
  boot  (rx)  : ORIGIN = 0x7C00, LENGTH = 510
  /* DOS .COM: CS=DS=ES=SS, PSP occupies 0x00-0xFF, code starts at 0x100. */
  com   (rx)  : ORIGIN = 0x0100, LENGTH = 0xFE00
  /* Working RAM for data/stack below the boot sector (conventional choice). */
  ram   (rw)  : ORIGIN = 0x0500, LENGTH = 0x7700
}

SECTIONS {
  /* Boot-sector image: everything in .text must fit 510 bytes. */
  .text  : { KEEP(*(.reset)) *(.text .text.*) } > boot
  .rodata : { *(.rodata .rodata.*) } > boot
  /* .COM image: packer redirects these to `com`; same order. */
  .data  : { *(.data .data.*) } > ram AT > boot
  .bss (NOLOAD) : { *(.bss .bss.*) *(COMMON) } > ram
  /* Packer checks: boot image total <= 510; com image total <= 0xFE00. */
}
