# ibm_ps2_model30 memory map

IBM Personal System/2 Model 30 (8530-002/021, 8 MHz 8086): the map as the compiler profile uses it.
Sources: IBM, *Personal System/2 Model 30 Technical Reference* (1987) for the machine; the BIOS
image itself (`ps2bios.bin`, revision of 09/02/86) for the ROM contents.

| Start | End | Size | Kind | Use |
|---|---|---|---|---|
| 00000h | 003FFh | 1 KiB | RAM | interrupt vector table |
| 00400h | 004FFh | 256 B | RAM | BIOS data area (0413h: memory size in KiB; 0410h: equipment word) |
| 00500h | 07BFFh | | RAM | boot-image `.bss` from 0500h, stack below 7C00h (`link.ld`) |
| 07C00h | 07DFFh | 512 B | RAM | boot sector (loaded by the BIOS) |
| 07E00h | 0FFFFh | | RAM | rest of the boot image (read by `runtime/crt0.s`) |
| 10000h | 9FFFFh | 576 KiB | RAM | above the tiny model's segment: far pointers, DOS programs |
| A0000h | AFFFFh | 64 KiB | video | MCGA memory (graphics modes) |
| B8000h | BFFFFh | 32 KiB | video | MCGA text and CGA-compatible modes |
| F0000h | FFFFFh | 64 KiB | ROM | BIOS (with Cassette BASIC); model byte FAh at FFFFEh, date at FFFF5h |

## Budgets

- Boot image (`link.ld`): boot sector 512 bytes; code and data 7E00h..FFFFh (33,280 bytes);
  `.bss` 0500h..24FFh (8 KiB); stack below 7C00h. The linker refuses an overflow.
- DOS `.COM`: one 64 KiB segment (64,000 bytes of image); `.EXE`: several segments in free memory.
- RAM a title can use beyond its segment: up to 640 KiB through far pointers (`ps2_info_rs` tests it).

ASSUMPTION (spec P-A1): the RAM and video layout are those of the Technical Reference; they are
checked by `ps2_info_rs` on the emulator (the BIOS's reported size, a write/read of every 16 KiB
block), not on hardware.
