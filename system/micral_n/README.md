# Micral N

CPU: Intel 8008. Backend: [`llvm-i8008`](../../llvm-i8008/README.md).

## History

The Micral N was built by R2E (Réalisations et Études Électroniques), France,
to a design led by François Gernelle, and delivered from 1973; it is often
cited as the first commercial microcomputer that was not a kit (Computer
History Museum timeline, 1973). Its MIC 01 monitor is documented in R2E's
*MICRAL N User's Manual* of January 1974. The association MO5 restored a
machine with floppy drive and published a card table and an emulator
(`platform/io_map.md`, [HxC], [MO5emu]). Month-level dates are not settled
by our sources.

## Machine summary

- CPU: Intel 8008 (the 8008-1 at 800 kHz in later machines; the RTL is not
  clock-accurate).
- RAM up to 0x37FF on the restored machine, a 256-byte boot EPROM over page
  0, the 2 KiB MIC 01 monitor EPROM at 0x3800 ([memory map](platform/memory_map.md)).
- Teletype, stack card (256-byte hardware LIFO), floppy controller
  ([I/O map](platform/io_map.md)).

## Simulators

- Primary: the ai_hw_engineer 8008 RTL (`design/intel_8008`) in a Micral N
  harness of our own (`platform/sim/`: teletype, stack card, disk, boot ROM
  mapping, after the MO5 emulator's device model). Build it with
  `platform/sim/build_rtl.sh` in the ai-hw-engineer image.
- Secondary: MO5's Virtual Micral N, not yet in `external/`.

## Software

| Title | Year | Original authors | Status | Folder |
|---|---|---|---|---|
| Boot ROM | 1973 (with the machine) | R2E | behavioral rewrite: 241 B of 256, loads and starts both programs below from a disk image on the RTL | [boot/](boot/) |
| MIC 01 monitor | 1974 (manual) | R2E | behavioral rewrite: 2040 B in its 2 KiB EPROM; host 7/7, RTL 7/7 from the ROM | [monitor/](monitor/) |
| Shooting Stars | 1976 | Willard I. Nico | byte for byte with the original: host 4/4, RTL 4/4 direct and booted, 2458 B | [shooting_stars/](shooting_stars/) |
| SCELBAL BASIC | 1974–75 | Mark G. Arnold, Nat Wadsworth | byte for byte with the original: host 10/10, RTL 10/10 direct and booted, 13826 B; program buffer 2560 B | [basic/](basic/) |

## Sources

- [R2E] *MICRAL N User's Manual*, January 1974 (in ai_hw_engineer
  `system/micral_n/spec/reference/`).
- [HxC] J.-F. Del Nero, *Restauration d'un Micral N* (hxc2001.com).
- [MO5emu] MO5, *Virtual Micral N*, MIT licence, github.com/Asso-MO5/virtual_micral_n.
- Computer History Museum, timeline of computer history, 1973.
