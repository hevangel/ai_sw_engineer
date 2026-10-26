/* Link script for the execution tests (system/ibm_pc/emulator/tests/i8086_codegen.rs):
 * everything in one RAM region from 0x1000; the test puts the stack at 0xF000
 * and a halt instruction at 0x0F00. */

MEMORY {
  ram (rwx) : ORIGIN = 0x1000, LENGTH = 0xD000
}

SECTIONS {
  .text   : { *(.text .text.*) } > ram
  .rodata : { *(.rodata .rodata.*) } > ram
  .data   : { *(.data .data.*) } > ram
  .bss (NOLOAD) : { *(.bss .bss.*) *(COMMON) } > ram
  /DISCARD/ : { *(.comment) }
}
