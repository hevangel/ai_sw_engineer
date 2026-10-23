/* busicom_141pf EXTENDED-board ("xl") target image linker script.
 *
 * Same syntax, ELF object format and address model as the stock link.ld
 * (ld.lld, e_machine = 0x4004, ELFCLASS32/LSB, llvm-i4004/spec/abi.md §5);
 * the ONLY differences are the widened ROM region and the larger data-RAM /
 * software-stack regions that the extra 4002 chips of the xl board provide.
 * Keep link.ld unchanged for the stock five-ROM / two-4002 board; use THIS
 * script only when targeting the xl board (platform/rtl/busicom_141pf_xl.sv
 * and run_image.sh --board xl), which the Rust firmware needs because the
 * runtime+crt0+HAL plus the firmware's static state do not fit the stock
 * 1280-byte ROM / 64-byte main RAM (see memory_map.md "Extended (xl) board").
 *
 * ROM: 16 x 4001 masks, chip-select = address bits 11:8, chip N at
 *      0x100*N..0x100*N+0xFF. Full architectural span 0x000-0xFFF, 4096 bytes.
 *      ROM ports of chips 0-2 keep their Busicom roles (shifter clocks /
 *      keyboard columns / drum index+paper); chips 3-15 have no I/O wired.
 *
 * RAM: up to 16 x 4002 across the four CM-RAM banks (DCL-selected), each 4002
 *      = 4 registers x (16 main + 4 status) nibbles. Data memory is NOT
 *      byte-flat-addressable from the CPU (abi.md §1.3): a datum is reached by
 *      an SRC pointer into {chip,reg,char}. We model the RAM address as the
 *      backend's 16-bit AS-0 handle [bank:2][chip:2][reg:2][char:4]
 *      (abi.md §1.3). Bank 0 keeps its two original 4002s for printer/lamps
 *      I/O (chip 0 = RAM0 role, chip 1 = RAM1 role); their MAIN characters are
 *      reserved for the firmware's own working registers exactly as on the
 *      stock board, so the linker-placed .data/.bss/stack use the ADDED chips
 *      (bank 0 chips 2-3, then banks 1-3) and never collide with firmware
 *      scratch or the lamp/printer port latches.
 */

MEMORY {
  /* Program ROM: 16 x 4001 = 4096 bytes on the xl board. */
  rom  (rx) : ORIGIN = 0x0000, LENGTH = 0x1000   /* 0x000-0xFFF, 4096 bytes */

  /* CM-RAM bank 0 only (issue #18: no DCL). The 16 registers of bank 0's four
   * 4002s (SRC address = [chip:2][reg:2][char:4]) are laid out by the firmware,
   * not the linker -- see memory_map.md "Extended (xl) board, bank-0 map":
   *   0x00-0x7F  Num registers WR DR CR SR TR MR T1 T2 (flags in status S3)
   *   0x80-0xBF  arithmetic scratch (mul / div / sqrt buffers)
   *   0xC0       key FIFO            0xE0  print register
   *   0xD0, 0xF0 per-function frames (packer overlay, below); status 0xF0 S0
   *              = key-down flag of the last keyboard scan pass
   * The firmware keeps all state in those registers, so `.data`/`.bss` must be
   * empty: the `ram` region has no room, and the packer fails the link if a
   * static appears. There is no software return stack (JMS/BBL calls; the
   * packer checks the 3-level hardware-stack depth). */
  ram    (rw) : ORIGIN = 0x00D0, LENGTH = 0x0000
  stack  (rw) : ORIGIN = 0x00F0, LENGTH = 0x0000
  frames (rw) : ORIGIN = 0x00D0, LENGTH = 0x0010   /* reg 0xD0, 16 chars */
  frames2 (rw) : ORIGIN = 0x00F0, LENGTH = 0x0010  /* reg 0xF0, 16 chars */

  /* Banks 2-3 (handles 0x0200..0x03FF, 512 more nibbles) are present on the
   * board and documented in memory_map.md as spare heap/arena headroom; they
   * are not carved into a named region here until a firmware build needs them
   * (add an `arena2` region and point a section at it then). */
}

SECTIONS {
  /* Reset/crt0 must sit at ROM address 0 (the 4004 fetches word 0 on reset). */
  .text   : { KEEP(*(.reset)) *(.text .text.*) } > rom
  .rodata : { *(.rodata .rodata.*) } > rom

  /* Initialised data is stored in ROM and copied to RAM by crt0 (AT > rom). */
  .data   : { *(.data .data.*) } > ram AT > rom
  .bss (NOLOAD) : { *(.bss .bss.*) *(COMMON) } > ram

  /* Software return stack region; SSP (P7) is initialised to __sstack_base. */
  .sstack (NOLOAD) : { __sstack_base = . ; . += LENGTH(stack) ; __sstack_top = . ; } > stack

  /* Status characters (AS-2) are opcode-addressed, not linker-placed. */

  /DISCARD/ : { *(.comment) *(.note .note.*) }
}

/* ROM budget guard: the xl board tops out at 16 x 4001 = 4096 bytes. */
ASSERT(. <= 0x1000, "busicom_141pf xl image exceeds the 4096-byte 16-ROM architectural max")
