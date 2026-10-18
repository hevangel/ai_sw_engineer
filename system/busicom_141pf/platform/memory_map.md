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
