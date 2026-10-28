/* atari_400_800 target image linker script — fill in from memory_map.md.
 * The syntax follows the linker used by llvm-mos6502 (ld.lld for LLVM-MC based
 * backends; llvm-mos uses its own extended ld.lld). */

MEMORY {
  rom (rx) : ORIGIN = 0x0000, LENGTH = 0x0000 /* TODO */
  ram (rw) : ORIGIN = 0x0000, LENGTH = 0x0000 /* TODO */
}

SECTIONS {
  .text   : { KEEP(*(.reset)) *(.text .text.*) } > rom
  .rodata : { *(.rodata .rodata.*) } > rom
  .data   : { *(.data .data.*) } > ram AT > rom
  .bss (NOLOAD) : { *(.bss .bss.*) *(COMMON) } > ram
}
