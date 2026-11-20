# Busicom 141-PF machine contract

The machine the emulator implements, written so the SystemVerilog system
(hevangel/ai_engineer#65) and this emulator can share one description. It
collects what `../../platform/memory_map.md`, `../../platform/io_map.md`,
`spec.md` and the ai_hw_engineer board spec already state. It does not depend
on RTL completion.

| Item | Contract | Source / status |
| --- | --- | --- |
| Machine | Busicom 141-PF printing calculator (sold as Unicom 141) | `../../README.md` |
| Board revision | Stock board with five 4001 and two 4002. The exact PCB revision and date are not known to this project (the ai_hw_engineer reconstruction dates it to October 1971 without a month) | Assumption A1 |
| CPU | Intel 4004, 4-bit, 12-bit program counter, 3-level hardware return stack, 16 index registers, 4-bit accumulator, carry | MCS-4 data sheet |
| Program ROM | 5 x 4001 = 1280 bytes at 0x000-0x4FF, chip selected by address bits 11:8. The emulator also accepts up to 4096 bytes (16 x 4001) for compiled images | `../../platform/memory_map.md` |
| Data RAM | 2 x 4002 (variant -1): 4 registers x (16 character + 4 status nibbles) each, addressed by SRC. 160 nibbles of working storage | same |
| Clock | About 740 kHz, 8 clocks per instruction cycle. One emulated instruction cycle is one chip `run_cycle`; a drum half-spin is 1481 of them | `../README.md`, `spec.md`; pacing is a reference value, not a motor model |
| Buses | 4-bit multiplexed data bus, CM-ROM and CM-RAM control lines, TEST pin input | MCS-4 data sheet |
| Reset | All chips rebuilt, PC 0, registers and RAM cleared, printer and keyboard state cleared. No reset circuit is modelled in more detail | `Emulator::reset` |
| Interrupts | None. The 4004 has no interrupt input and the firmware does not use one | MCS-4 data sheet |
| Inputs | keyboard matrix (8 rows x 4 columns) through 4003 #0 scan and ROM1 column nibble; decimal selector (0-6, 8) and rounding selector (0, 1, 8) on scan rows 8 and 9; paper-advance button on ROM2 bit 3; drum sector (TEST) and drum index (ROM2 bit 0) | `../../platform/io_map.md` |
| Outputs | Drum printer: 15 numeric columns plus two symbol columns, 13-sector drum, hammer fire, paper advance, red ink; three lamps (memory, overflow, negative) on RAM1 port | same |
| Media | None. The firmware is a mask ROM; there is no disk, tape or boot loader. The original ROM is supplied by the user (SHA-256 `c6af325d64b0895a8c6f7298e5443bbf5814783fabd6a1bcd2df08a9bec2ceb7`) | `../../firmware/original/provenance.md` |
| Display, audio | The printed tape and the lamps are the only output. No display, no audio | |
| Software layers | Firmware only; no OS (see hevangel/ai_engineer#66, closed) | |

## Unsupported options

XL board (16 x 4002 with bank switching), more than two RAM chips, a graphical
front panel, drum motor dynamics, transistor-level timing and the optional
parts of the 141-PF family (other models). The platform runner handles the XL
board.

## Assumptions

- **A1.** Board revision: the stock five-ROM, two-RAM board from the original
  firmware release stands in for the production unit.
- **A2.** One drum half-spin is 1481 instruction cycles; this is the value the
  committed oracle was captured at. The RTL also reproduces identical rows
  down to 740 (platform README).
- **A3.** The pinned `chips` core has inverted JCN TEST polarity; the board
  wiring here compensates, as documented in `../../platform/README.md`.
