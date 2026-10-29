# ibm_pc memory map

Real mode, 20-bit physical addresses (`segment * 16 + offset`). The map is the 64 KiB CGA 5150 modeled by `$AI_HW_ENGINEER/system/ibm_pc`.

| Start | End | Size | Kind | Use |
|---|---|---|---|---|
| 00000h | 003FFh | 1 KiB | RAM | interrupt vector table: 256 vectors x (offset, segment) |
| 00400h | 004FFh | 256 B | RAM | BIOS data area (the BIOS owns it; a title only reads it) |
| 00500h | 07BFFh | 30,464 B | RAM | free: title data, bss and stack (stack grows down from 7C00h) |
| 07C00h | 07DFFh | 512 B | RAM | boot sector, loaded here by the BIOS from sector 1 of the boot drive |
| 07E00h | 0FFFFh | 33,280 B | RAM | free: title image beyond the boot sector, loaded by the title's own stage 2 |
| 10000h | B7FFFh | n/a | none | not populated in the 64 KiB configuration (reads return FFh) |
| B8000h | BBFFFh | 16 KiB | RAM (CGA) | text and graphics memory; adapter aliases decode the same 16 KiB |
| F6000h | FDFFFh | 32 KiB | ROM | Cassette BASIC |
| FE000h | FFFFFh | 8 KiB | ROM | BIOS; reset vector FFFF0h (CS:IP = FFFF:0000) |

ROM writes from the CPU are ignored. Unmapped memory and I/O read FFh.

## Budgets

- ROM available to a title: none. A title is not in ROM; it is a program image in RAM. Its code, constants and initialized data count against the same RAM as its bss and stack.
- RAM available to a title (incl. stack): 00500h..0FFFFh minus the 512 B boot sector region, about 62 KiB in total (63,744 B), of which one boot sector (512 B) is loaded by the BIOS and the rest by the title's own loader. A title that must also leave DOS intact (a `.COM` program) has about 64 KiB minus the DOS image; the first platform targets the bare boot path instead.
- Target image form (decision P1, open): bootable 160 KiB floppy image, boot sector at 0000:7C00 with the 55AAh signature at offset 510. A `.COM`-under-DOS form is a later variant.

Source: IBM Personal Computer Technical Reference (August 1981), memory map and BIOS listing; `$AI_HW_ENGINEER/system/ibm_pc/spec/spec.md` (RAM, CGA, BASIC and BIOS ranges, FFh for unmapped). The interrupt-vector, BIOS-data-area and boot-sector addresses are 8086/BIOS conventions; page references into the Technical Reference are to be added when the memory-map document is checked against the PDF.
