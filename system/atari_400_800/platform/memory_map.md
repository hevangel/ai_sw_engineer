# atari_400_800 memory map

Stock Atari 400/800 (NTSC). All addresses are CPU addresses on the 6502 bus.

| Start | End | Size | Kind | Use |
|---|---|---|---|---|
| $0000 | $00FF | 256 B | RAM | zero page |
| $0100 | $01FF | 256 B | RAM | 6502 stack |
| $0200 | $05FF | 1 KiB | RAM | OS / user page 2-5 |
| $0600 | $3FFF ($7FFF / $BFFF) | 16 / 32 / 48 KiB | RAM | general (profile-dependent top) |
| $A000 | $BFFF | 8 KiB | ROM (cartridge) | cartridge slot B (RAM on a 48K machine without a cartridge) |
| $8000 | $9FFF | 8 KiB | ROM (cartridge) | cartridge slot A (16 KiB carts only; RAM otherwise) |
| $C000 | $CFFF | 4 KiB | unused | floats high (emulator returns $FF, A8-K3) |
| $D000 | $D0FF | 256 B | I/O | GTIA (mirrored every 32 B through $D1FF) |
| $D200 | $D2FF | 256 B | I/O | POKEY (mirrored every 16 B) |
| $D300 | $D3FF | 256 B | I/O | PIA 6520 (mirrored every 4 B) |
| $D400 | $D4FF | 256 B | I/O | ANTIC (mirrored every 16 B) |
| $D500 | $D7FF | 3 KiB | unused | floats high (emulator returns $FF, A8-K3) |
| $D800 | $DFFF | 2 KiB | ROM | floating-point package (Atari math ROM) |
| $E000 | $FFFF | 8 KiB | ROM | OS ROM (vectors at $FFFA-$FFFF) |

Stock RAM configurations: Atari 400 = 16 KiB ($0000-$3FFF), Atari 800 = 48 KiB
($0000-$BFFF, minus the cartridge overlay when a cartridge is inserted).

## Budgets

- ROM available to a title: 8 KiB cartridge at $A000 (16 KiB at $8000-$BFFF
  when no RAM-based title needs the bank).
- RAM available to a title (incl. stack / zero page): 16 KiB (Atari 400) or
  up to 48 KiB minus the cartridge overlay (Atari 800). A title's target
  image must state the machine profile it assumes.

Source for every row: Atari, Inc., *Atari 400/800 Technical Reference Notes*
(memory map chapter) and *De Re Atari* (1981), appendix memory map; register
mirror spacing per the GTIA/POKEY/PIA/ANTIC chip data sheets reprinted in the
Technical Reference Notes.
