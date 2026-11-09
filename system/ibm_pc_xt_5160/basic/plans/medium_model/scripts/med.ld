ENTRY(_start)
MEMORY {
  main (rwx) : ORIGIN = 0x0100, LENGTH = 0xFF00
  far1 (rwx) : ORIGIN = 0x0000, LENGTH = 0xFFF0, SEGMENT = AUTO
  far2 (rwx) : ORIGIN = 0x0000, LENGTH = 0xFFF0, SEGMENT = AUTO
}
SECTIONS {
  .text   : { KEEP(*(.text.start)) *(.text .text.*) } > main
  .rodata : { *(.rodata .rodata.*) } > main
  .data   : { *(.data .data.*) } > main
  .bss (NOLOAD) : { *(.bss .bss.*) *(COMMON) } > main
  .far1   : { *(.far.text.1.*) } > far1
  .far2   : { *(.far.text.2.*) } > far2
  .endmark : { KEEP(*(.endmark)) } > far2
}
