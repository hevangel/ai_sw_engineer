# ibm_pc I/O map

Port-mapped I/O through `IN`/`OUT` (8-bit, address in DX or an immediate byte). Decode as in the 5150 and in `$AI_HW_ENGINEER/system/ibm_pc`; unmapped ports read FFh and absent serial, printer, game and expansion adapters use their documented absent-device values.

| Address / port | Direction | Device | Behavior | Source |
|---|---|---|---|---|
| 00h-0Fh | R/W | Intel 8237A DMA | channel 0 memory refresh, channel 2 diskette transfers | hw spec; Tech Ref |
| 20h-21h | R/W | Intel 8259A PIC | IRQ0 timer, IRQ1 keyboard, IRQ6 diskette; port 21h is the interrupt mask | hw spec; Tech Ref |
| 40h-43h | R/W | Intel 8253 PIT | channel 0 system timer (IRQ0), channel 1 DRAM refresh request, channel 2 speaker tone; 43h is the control word | hw spec; Tech Ref |
| 60h-63h | R/W | Intel 8255 PPI | 60h keyboard scan code, 61h speaker gate and keyboard acknowledge, 62h switches, 63h control word | hw spec; Tech Ref |
| 80h-83h | W | DMA page latches | upper address bits for the DMA channels | hw spec |
| A0h | W | NMI mask | enables or disables NMI | hw spec |
| 3D0h-3DFh | R/W | CGA (Motorola 6845) | 6845 index/data at 3D4h/3D5h, mode control 3D8h, color select 3D9h, status 3DAh (display enable, vertical retrace) | hw spec; Tech Ref |
| 3F0h-3F7h | R/W | Intel 8272 FDC | digital output register 3F2h, main status 3F4h, data 3F5h | hw spec; Tech Ref |

Interrupt use that a title relies on: INT 08h timer tick (IRQ0), INT 09h keyboard (IRQ1), INT 0Eh diskette (IRQ6); BIOS services INT 10h video, INT 13h diskette, INT 16h keyboard. These are BIOS software conventions, not hardware; a title that uses the BIOS depends on the original IBM BIOS image in the oracle.

ASSUMPTION entries (behavior the documentation does not state) go here too, each with how it will be validated.

The planar decodes only A9:A5, so each range above repeats at higher addresses (`port & 3FFh` selects the device). 10h-1Fh and every other port not listed reads FFh.

## Register details the emulator implements

Source for all of this: IBM Technical Reference (August 1981) and the datasheets named in `../emulator/spec/spec.md`; page references are still to be added.

**8255 PPI** (the BIOS writes control word 99h: port A in, port B out, port C in):

| Port | Bits |
|---|---|
| 60h A (read) | PB7 = 0: the keyboard scan code latch. PB7 = 1: DIP switch bank SW1 |
| 61h B (write, read back) | 0 timer-2 gate; 1 speaker data; 6 keyboard clock enable (held low 20 ms or more, then high, resets the keyboard, which answers AAh); 7 clear the keyboard latch and IRQ1 / select SW1 on port A |
| 62h C (read) | 5 timer-2 output; 4 cassette data in (timer 2 output when no tape); 3:0 SW2 (0); 7:6 parity and I/O-channel check (0) |

SW1 = 6Fh on the modeled machine: bit 0 diskette drives present; bits 7:6 = 01 two drives; bits 5:4 = 10 80x25 colour display; bit 1 no 8087; bits 3:2 = 11 64 KiB on the planar.

**8237A DMA**: ports 00h-07h channel address and count (byte flip-flop), 08h command (write) and status (read), 09h request, 0Ah single mask, 0Bh mode, 0Ch clear flip-flop, 0Dh master clear (write) and temporary (read), 0Eh clear mask, 0Fh write all masks. Page latches 81h channel 2 (diskette), 82h channel 3, 83h channel 1 (the planar has no carry from the 16-bit address into the page). Channel 0 is memory refresh.

**Diskette adapter**: 3F2h digital output register: bits 1:0 drive select, bit 2 controller /RESET (0 = reset), bit 3 DMA and IRQ6 enable, bits 4-7 motor on for drives 0-3. 3F4h 8272 main status: bit 7 RQM, bit 6 DIO (1 = controller to CPU), bit 4 busy, bits 3:0 drive seeking. 3F5h data. Commands: SPECIFY 03h, SENSE DRIVE STATUS 04h, WRITE DATA 05h, READ DATA 06h, RECALIBRATE 07h, SENSE INTERRUPT STATUS 08h, READ ID 0Ah, FORMAT TRACK 0Dh, SEEK 0Fh; the high bits of the command byte are MT, MFM, SK.

**CGA**: 3D4h index and 3D5h data of the 6845 (registers 12-13 start address, 14-15 cursor); 3D8h mode (bit 0 80-column text, bit 1 graphics, bit 2 black-and-white, bit 3 video enable, bit 4 640x200, bit 5 blink); 3D9h colour select; 3DAh status (bit 0 display disabled by blanking, bit 3 vertical retrace).

Behavior the documentation does not state is in the emulator's assumption ledger (`../emulator/spec/spec.md`, E-A8 to E-A12): keyboard spacing and reset timing, the single reset polling interrupt of the diskette controller, fixed floppy timings, CGA frame timing, the switch settings.
