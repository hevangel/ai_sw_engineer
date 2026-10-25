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

The specific register bit meanings (PPI port B and C, CGA mode bits, FDC command set) are not restated here yet; they are filled from the Technical Reference when the first title needs a given device, so each row below its first use carries a page reference.
