/* ibm_pc target image linker script (decision P1 in memory_map.md: bootable floppy).
 * Real mode, tiny model: code, data and stack share segment 0, so every
 * address below is both the offset and the physical address.
 * The syntax follows the linker used by llvm-i8086 (ld.lld, LLVM MC). */

MEMORY {
  /* The BIOS loads the first 512 bytes of the boot drive here and jumps to it. */
  boot (rwx) : ORIGIN = 0x7C00, LENGTH = 0x0200
  /* Remaining image, loaded by the title's stage 2 (zero-length until a title needs it). */
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
