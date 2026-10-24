# Micral N boot ROM — behavior spec

The 256-byte bootstrap EPROM (1702A) on the 4 KiB RAM card at 0x0000. MO5
dumped it (`MIC_1_EPROM_CARTE_MEM_4K.BIN`, SHA-256 `19D8FCBB…D31DB`, HxC
2001 page) but did not publish its bytes, so this is a **behavioral rewrite**.

Sources:

- MO5's description: the boot program reads the first 14 KiB of the floppy,
  checks checksums, stores the data from address 0, then restarts the machine
  with the boot ROM switched off (MO5 articles "Programmer le Micral N",
  "Lecture et sauvegarde des ROMs du Micral N", 2023; quoted in search results
  on 2026-10-23, pages now 404).
- HxC: the disk format the bootloader handles: 64 tracks, 32 × 128-byte
  sectors, hard-sectored FM; the stack card serves as the bootloader's stack.
- MO5 emulator (`../platform/io_map.md` [MO5emu]): the controller's I/O
  protocol, the sector layout and checksum, the ROM switch on `RST 0`, and
  the image's first sector (track 0, sector 16).

## Behavior

1. Reset starts the CPU at 0x0000 with the boot ROM mapped over page 0.
2. Step the head out until the status says track 0 (STEP pulses with DIR 0).
3. Read sectors in order from track 0 sector 16 (ASSUMPTION BOOT-1), 32
   sectors per track, stepping in one track after sector 31.
4. For each sector: select it, set the channel pointer to 0 and the count to
   131 bytes, start the transfer, wait until the channel pointer reaches 131,
   then pop the bytes. They come out last first: the checksum, then the 128
   data bytes backwards. The data goes onto the stack card, which reverses
   them again; popping it gives the bytes in disk order, and each is stored
   at the next 128-byte block of RAM from address 0 while the checksum is
   computed. The boot cannot read its stores back: while it is mapped, page-0
   reads return the boot ROM. This is why the stack card is "the bootloader's
   stack" (HxC).
5. On a checksum mismatch, select another sector and then the same one again,
   and re-read it (ASSUMPTION BOOT-2).
6. After 0x3700 bytes (ASSUMPTION BOOT-3), execute `RST 0`. That fetch
   switches the boot ROM off, and the loaded program runs from address 0.

## Assumption ledger

| ID | Assumption | Why |
|---|---|---|
| BOOT-1 | The image starts at track 0, sector 16 | [MO5emu] `create_virtual_disk` puts 16 filler sectors before the program |
| BOOT-2 | A bad sector is re-read until it checks; reselecting it restarts the controller's byte index | the controller model restarts a sector only when the sector number changes |
| BOOT-3 | 0x3700 bytes (110 sectors), not the full 14 KiB, are loaded; the boot's frames use 0x3700–0x37FF | a compiled boot needs RAM for its frames; the original kept its state in registers and the stack card |
| BOOT-4 | Transfer completion is detected by polling the channel pointer (`INP 0` with A = 0xFF) | the status bits are tentative in [MO5emu]; the pointer is not |
