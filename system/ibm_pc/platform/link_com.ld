/* ibm_pc DOS program (.COM) linker script.
 * PC DOS loads a .COM file at offset 100h of a segment that starts with the
 * 256-byte program segment prefix, with CS = DS = ES = SS; the program owns the
 * rest of the segment. Tiny model, as link.ld: every address is an offset.
 * `i8086-ld --bin x.com` writes the loaded sections: that file is the .COM.
 * The stack is set by runtime/crt0_com.s from the top of memory in the PSP. */

ENTRY(_start)

MEMORY {
  com (rwx) : ORIGIN = 0x0100, LENGTH = 0xFE00
}

SECTIONS {
  .text   : { KEEP(*(.text.start)) *(.text .text.*) } > com
  .rodata : { *(.rodata .rodata.*) } > com
  .data   : { *(.data .data.*) } > com
  .bss (NOLOAD) : { *(.bss .bss.*) *(COMMON) } > com
}
