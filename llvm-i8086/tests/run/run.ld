/* Link script for the execution tests (system/ibm_pc/emulator/tests/i8086_codegen.rs):
 * everything in one RAM region from 0x1000; the test puts the stack at 0xF000
 * and a halt instruction at 0x0F00. Code and data in sections `.far.*` are in a
 * segment of their own, the first paragraph after the rest (segments.ll). */

MEMORY {
  ram (rwx) : ORIGIN = 0x1000, LENGTH = 0xD000
  far (rwx) : ORIGIN = 0x0000, LENGTH = 0x4000, SEGMENT = AUTO
}

SECTIONS {
  .text   : { *(.text .text.*) } > ram
  .rodata : { *(.rodata .rodata.*) } > ram
  .data   : { *(.data .data.*) } > ram
  .bss (NOLOAD) : { *(.bss .bss.*) *(COMMON) } > ram
  .far    : { *(.far.text) *(.far.data) } > far
  /DISCARD/ : { *(.comment) }
}
