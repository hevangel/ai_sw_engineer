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

  /* Data RAM arena: bank 0 chips 2-3 (the two ADDED 4002s of bank 0), main
   * characters. AS-0 handles 0x0020..0x005F =
   * [bank0][chip2..3][reg0..3][char0..15] = 128 nibbles. Bank 0 chips 0-1
   * (handles 0x0000..0x001F) are left to the firmware working set + the
   * printer/lamp port chips, matching the stock layout. */
  ram  (rw) : ORIGIN = 0x0020, LENGTH = 0x0040   /* bank0 chips 2-3 main, 128 nibbles */

  /* Software return stack: CM-RAM BANK 0, SRC 0x80..0xFF (128 nibbles).
   * MUST be in bank 0 and within an 8-bit SRC address: SSP is P7 = R14:R15, a
   * single 8-bit pair, so `fim p7, __sstack_base` can only load an 8-bit base
   * and the push/pop helpers address the stack with a plain SRC under the
   * current DCL bank (bank 0, set by crt0). An earlier layout put the stack at
   * handle 0x100 (CM-RAM bank 1); FIM truncated that to 0x00, so the stack grew
   * from SRC 0x00 over the firmware's working registers and the printer/lamp
   * port chips -> the calculator produced no tape. Bank-0 0x80..0xFF is clear
   * of the working set / I/O (0x00..0x1F), the (empty) data arena (0x20..0x5F)
   * and the per-function frame overlay (packer-placed from 0x20 up, a few dozen
   * chars). Depth-16 x (<=8 nibbles/far call) fits 128 nibbles. */
  stack (rw) : ORIGIN = 0x0080, LENGTH = 0x0080  /* bank0 SRC 0x80..0xFF, 128 nibbles */

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
