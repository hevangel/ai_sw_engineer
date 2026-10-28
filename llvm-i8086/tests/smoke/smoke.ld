/* Link script for the Rust smoke programs run on the bare 8088 core
 * (system/ibm_pc/emulator/tests/i8086_rust.rs): everything in one RAM region
 * from 0x0600, above the interrupt vectors and the BIOS data area; the test puts
 * the stack at 0xFFF0 and the region ends at 0xF000, so the stack has 4 KiB. */

MEMORY {
  ram (rwx) : ORIGIN = 0x0600, LENGTH = 0xEA00
}

SECTIONS {
  .text   : { *(.text .text.*) } > ram
  .rodata : { *(.rodata .rodata.*) } > ram
  .data   : { *(.data .data.*) } > ram
  .bss (NOLOAD) : { *(.bss .bss.*) *(COMMON) } > ram
  /DISCARD/ : { *(.comment) }
}
