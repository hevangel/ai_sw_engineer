# Boot ROM final report

| Check | Result |
|---|---|
| Host tests | 2/2 (`tests/disk_model.rs`) |
| Image size vs EPROM | 245 / 256 bytes |
| RAM | frames 0x3700–0x3705 (BOOT-3) |
| Call depth | 2 of 7 |
| RTL: boot Shooting Stars | 4/4 transcripts |
| RTL: boot SCELBAL | 10/10 transcripts |
| Secondary emulator | not run |

## Known gaps and limits

- Loads 0x3700 bytes, not 14 KiB (BOOT-3): a compiled boot needs a page for
  its frames. Programs must end below 0x3700 (platform `memory_map.md`).
- Behavioral, not byte-identical: the original EPROM is unpublished.

## Lessons

- While the boot ROM is mapped, page-0 reads return the ROM; the checksum is
  computed as bytes pass through the stack card, which is why the original
  needed the card as its stack.
