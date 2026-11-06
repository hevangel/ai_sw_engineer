# ibm_ps2_model30 I/O map

The compiler profile does not program devices directly: its programs use the BIOS (INT 10h video,
INT 13h diskette, INT 16h keyboard, INT 1Ah clock) and DOS (INT 21h). The ports below are the ones
the shared runtime touches. Source: IBM, *Personal System/2 Model 30 Technical Reference* (1987).

| Port | Device | Used by |
|---|---|---|
| 20h, 21h | 8259A interrupt controller (EOI) | `irq_rs` (`out 20h, 20h` at the end of a handler) |
| 40h-43h | 8253 timer | the BIOS timer tick (IRQ 0) that `irq_rs` counts |
| 60h | keyboard data | `irq_rs` reads scan codes |
| 61h | system control port B | `irq_rs` acknowledges the keyboard the 5150 way (bit 7 pulse); ASSUMPTION (spec P-A2): harmless on the Model 30, to be checked when `irq_rs` runs on the emulator |
| 3D0h-3DFh | MCGA (CRTC, mode, status) | the BIOS only |
| 3F0h-3F7h | diskette controller | the BIOS only |
