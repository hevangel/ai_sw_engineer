# atari_400_800

CPU: mos6502 (Atari "SALLY" 6502C). Backend: `llvm-mos6502`.

## History

The Atari 400 and 800 shipped in 1979 (announced in late 1978) as Atari, Inc.'s
first home-computer line: a cartridge/ROM-based machine built around custom
video, graphics and sound chips (ANTIC, GTIA, POKEY) with the CPU deliberately
subordinate to the display pipeline. Month-level claims are not made here; the
1979 introduction year follows the Computer History Museum timeline and the
machine overviews below.

## Machine summary

- CPU and clock: MOS 6502 core (Atari CO14806 "SALLY", labeled 6502C — not a
  WDC 65C02) at 1.7897725 MHz NTSC (PHI2 = color clock / 2). The SALLY adds a
  HALT line the custom chips use for DMA; see emulator spec assumption A8-K1.
- ROM / RAM: 48 KiB RAM in the 800, 16 KiB in the 400; 10 KiB OS ROM
  ($E000-$FFFF plus the floating-point package at $D800-$DFFF); 8 KiB
  cartridge slot at $A000 (16 KiB carts extend to $8000).
- I/O: GTIA (video output, players/missiles, console keys, triggers), POKEY
  (sound, keyboard, serial/SIO, random), PIA 6520 (joystick ports), ANTIC
  (display list processor, DMA). See [platform/io_map.md](platform/io_map.md).
- Memory map: see [platform/memory_map.md](platform/memory_map.md)

## Simulators

- Primary (ai_hw_engineer): CPU oracle at `$AI_HW_ENGINEER/design/mos_6502`
  (NMOS 6502 RTL); the whole-machine SV build (`system/atari_800`) is planned
  under [ai_engineer#355](https://github.com/hevangel/ai_engineer/issues/355)
  and does not exist yet.
- Secondary (external/): no third-party Atari 8-bit emulator is pinned yet;
  the machine-level oracle plan (Altirra / atari800 / MAME) is recorded in
  [emulator/spec/spec.md](emulator/spec/spec.md).

## Software

| Title | Year | Original authors | Status | Folder |
|---|---|---|---|---|
| Atari OS rev. B | 1979 | Atari, Inc. | planned ([issue #139](https://github.com/hevangel/ai_sw_engineer/issues/139)) | — |
| Atari BASIC rev. C | 1983 | Shepardson Microsystems | planned ([issue #140](https://github.com/hevangel/ai_sw_engineer/issues/140)) | — |
| Atari DOS 2.0S | 1981 | Atari / Shepardson Microsystems | planned ([issue #141](https://github.com/hevangel/ai_sw_engineer/issues/141)) | — |
| Star Raiders | 1980 | Atari, Inc. (Doug Neubacher) | planned ([issue #142](https://github.com/hevangel/ai_sw_engineer/issues/142)) | — |

## Sources

- [ai_engineer#170](https://github.com/hevangel/ai_sw_engineer/issues/170) — emulator issue (scope and acceptance)
- [ai_engineer#355](https://github.com/hevangel/ai_engineer/issues/355) — system spec / SV simulation issue (machine contract shared with it)
- Computer History Museum, "Timeline of Computer History" (1979, Atari 400/800 introduction year)
- Atari, Inc., *Atari 400/800 Technical Reference Notes* and *De Re Atari* (1981) — memory map, hardware registers
- */home/Primary: per-file source URLs and SHA-256 hashes for downloaded reference material go under `reference/` when the doc-clean step runs*
