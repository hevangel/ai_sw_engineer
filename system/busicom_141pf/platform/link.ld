/* busicom_141pf target image linker script.
 *
 * Syntax: ld.lld (the linker llvm-i4004 uses for its LLVM-MC backend; the
 * ELF object format is e_machine = 0x4004, ELFCLASS32, ELFDATA2LSB,
 * llvm-i4004/spec/abi.md §5). Addresses are byte addresses in the two separate
 * MCS-4 spaces; there is NO unified memory map (memory_map.md), so program and
 * data live in distinct regions the backend reaches through different opcodes
 * (ROM fetch / FIN vs SRC+RDM/WRM), never a single flat pointer.
 *
 * ROM: five 4001 masks on the Busicom board, chip-select = address bits 11:8,
 *      so chip N occupies 0x100*N .. 0x100*N+0xFF. The original fits in the
 *      first five (0x000-0x4FF, 1280 bytes). LENGTH is the 5-ROM board limit;
 *      raise ORIGIN/LENGTH only for a >5-ROM board (architectural max 16 x
 *      4001 = 0x000-0xFFF, 4096 bytes).
 *
 * RAM: two 4002s on CM-RAM line 0 (P0 = 0 and 1). A 4002 has 4 registers x
 *      (16 main + 4 status) nibbles. Data memory is NOT byte-flat-addressable
 *      from the CPU (abi.md §1.3): a datum is reached by an SRC pointer into
 *      {chip,reg,char}. The linker therefore only needs to carve the writable
 *      region into (a) a .data/.bss arena and (b) the software return-stack
 *      region (abi.md §4.4, P7/SSP, grows upward). We model the RAM address as
 *      the backend's 16-bit AS-0 handle [bank:2][chip:2][reg:2][char:4]
 *      (abi.md §1.3); with both 4002s on bank 0 the usable main-character
 *      handles are chip 0 reg 0..3 and chip 1 reg 0..3, char 0..15.
 */

MEMORY {
  /* Program ROM: 5 x 4001 = 1280 bytes on the Busicom board. */
  rom  (rx) : ORIGIN = 0x0000, LENGTH = 0x0500   /* 0x000-0x4FF, 1280 bytes */

  /* Data RAM arena: 4002 #0 (chip 0), main characters, AS-0 handles
   * 0x0000..0x003F = [bank0][chip0][reg0..3][char0..15] = 64 nibbles. */
  ram  (rw) : ORIGIN = 0x0000, LENGTH = 0x0040   /* 4002 #0 main, 64 nibbles */

  /* Software return stack: 4002 #1 (chip 1), main characters, AS-0 handles
   * 0x0040..0x007F = [bank0][chip1][reg0..3][char0..15] = 64 nibbles. Grows
   * upward from the base (abi.md §4.4). Kept physically separate from the
   * .data/.bss arena so a deep call graph cannot corrupt live variables.
   * The Busicom firmware's own graph is <= 3 deep (hardware stack only), so
   * for the yardstick image this region is reserved but unused. */
  stack (rw) : ORIGIN = 0x0040, LENGTH = 0x0040  /* 4002 #1 main, 64 nibbles */
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

  /* Status characters (AS-2) are opcode-addressed, not linker-placed; they are
   * named in the backend, not here. */

  /DISCARD/ : { *(.comment) *(.note .note.*) }
}

/* ROM budget guard: the five-ROM board cannot hold more than 1280 bytes. */
ASSERT(. <= 0x0500, "busicom_141pf image exceeds the 1280-byte 5-ROM budget; add 4001s and widen rom in link.ld (max 4096)")
