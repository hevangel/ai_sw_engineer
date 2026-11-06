/* IBM PS/2 Model 30 target image linker script: a bootable 720 KiB diskette
 * (memory_map.md). Real mode, tiny model, the same layout as the 5150 profile's
 * system/ibm_pc/platform/link.ld: code, data and stack share segment 0, so every
 * address below is both the offset and the physical address.
 * Linked by llvm-i8086/asm (i8086-ld); `i8086-ld --sign --floppy x.img
 * --floppy-kib 720` packs the result. */

MEMORY {
  /* The BIOS loads the first 512 bytes of the boot drive here and jumps to it. */
  boot (rwx) : ORIGIN = 0x7C00, LENGTH = 0x0200
  /* The rest of the image, read by crt0.s. */
  img  (rwx) : ORIGIN = 0x7E00, LENGTH = 0x8200
  /* Data below the boot sector; the stack starts at 0x7C00 and grows down. */
  ram  (rw)  : ORIGIN = 0x0500, LENGTH = 0x2000
}

SECTIONS {
  .boot   : { KEEP(*(.boot)) *(.text.boot) } > boot
  .text   : { *(.text .text.*) } > img
  .rodata : { *(.rodata .rodata.*) } > img
  .data   : { *(.data .data.*) } > img
  .bss (NOLOAD) : { *(.bss .bss.*) *(COMMON) } > ram
  _stack_top = 0x7C00;
}
