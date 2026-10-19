# Busicom 141-PF memory map

MCS-4 has separate address spaces; there is no unified memory map.

| Space | Range | Size | Devices | Use |
|---|---|---|---|---|
| Program ROM | 0x000–0x4FF | 1280 bytes | 4001 #0–#4 (chip select = address bits 11:8) | firmware |
| Program ROM (architectural max) | 0x000–0xFFF | 4096 bytes | up to 16 × 4001 | headroom for a Rust build that does not fit 1280 bytes |
| RAM main characters | 2 chips × 4 registers × 16 nibbles | 128 nibbles (64 bytes) | 4002 #0, #1 (CM-RAM0, P0 = 0/1) | working registers, memory |
| RAM status characters | 2 × 4 × 4 nibbles | 32 nibbles | same | flags, sign, decimal position |
| CPU index registers | R0–R15 | 16 nibbles | 4004 | scratch, pointers |
| CPU stack | 3 levels | — | 4004 | subroutine return addresses only, not addressable |

## Budgets

- ROM: **1280 bytes** to match the original; 4096 bytes is the hardware ceiling with more 4001s. The title report states size against both.
- RAM: 160 nibbles (128 main + 32 status) plus 16 index registers. Call depth: 3, including any interrupt-free helper the backend emits.

Sources: MCS-4 data sheet (Nov 1971) and the ai_hw_engineer board spec §2 (chip count and straps).

## Linker regions (`link.ld`)

The CPU cannot flat-address data RAM; a datum is reached by an `SRC` pointer
into `{chip, reg, char}` (`llvm-i4004/spec/abi.md` §1.3), so the linker only
carves the writable space into two non-overlapping regions, modelled as the
backend's 16-bit AS-0 handle `[bank:2][chip:2][reg:2][char:4]`:

| Region | Handle range | Device | Size | Use |
|---|---|---|---|---|
| `rom` | 0x000–0x4FF | 4001 #0–#4 | 1280 B | `.reset` (at 0), `.text`, `.rodata`, `.data` load image |
| `ram` | 0x0000–0x003F | 4002 #0 main | 64 nibbles | `.data` (copied from ROM by crt0) + `.bss` arena |
| `stack` | 0x0040–0x007F | 4002 #1 main | 64 nibbles | software return stack (abi.md §4.4, P7/SSP, grows upward) |

- The software return stack is kept in a **separate 4002** from the `.data`/
  `.bss` arena so a deep call graph cannot corrupt live variables. `__sstack_base`
  / `__sstack_top` bound it; crt0 initialises P7 (SSP) to `__sstack_base`.
- The Busicom firmware's own call graph is ≤ 3 deep, so it uses only the 4004
  **hardware** return stack and never touches the software stack region — that
  region is reserved for a Rust build whose static call depth can exceed 3
  (abi.md §4.4 two-tier scheme).
- RAM **status** characters (AS-2) are opcode-addressed, not linker-placed; the
  backend names them directly.
- `link.ld` ends with an `ASSERT` that fails the link if the image exceeds the
  1280-byte five-ROM budget; widen `rom` (and add 4001 straps on the board) only
  for a larger board, up to the 4096-byte architectural max.
