/* ibm_pc DOS program with more than one segment (.EXE, i8086-ld --exe).
 * The first segment is the program segment as in link_com.ld: offset 100h up, with
 * the entry point, the data and the stack of the program (`__stack_top`, defined by
 * the program; the header's SS:SP). Sections named `.far.text` and `.far.data` go into
 * a segment of their own, which starts at the first paragraph after the first one's
 * contents; `sym@seg` is that paragraph, and every such word is in the .EXE's
 * relocation table, so DOS adds the segment it loaded the file at. */

ENTRY(_start)

MEMORY {
  main (rwx) : ORIGIN = 0x0100, LENGTH = 0xE000
  far  (rwx) : ORIGIN = 0x0000, LENGTH = 0x4000, SEGMENT = AUTO
}

SECTIONS {
  .text   : { KEEP(*(.text.start)) *(.text .text.*) } > main
  .rodata : { *(.rodata .rodata.*) } > main
  .data   : { *(.data .data.*) } > main
  .bss (NOLOAD) : { *(.bss .bss.*) *(COMMON) } > main
  .far    : { *(.far.text) *(.far.data) } > far
}
