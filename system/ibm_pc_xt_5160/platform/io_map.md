# ibm_pc_xt_5160 I/O map

Ports, interrupts and DMA as target software sees them. Authority: IBM 5160
Technical Reference (Apr 1983) via the ai_hw_engineer XT/5150 platforms;
original BIOS/DOS behavior is the oracle. This map covers what the first
titles and the crt0 need; DMA/PIT/PPI programming detail stays with the
hardware specs until a title needs it.

| Address / port | Direction | Device | Behavior | Source |
|---|---|---|---|---|
| 020h–021h | R/W | 8259 PIC | IRQ0–7 mask/dispatch; vectors 08h–0Fh | 5160 Tech Ref |
| 040h–043h | R/W | 8253 PIT | counter 0 = IRQ0 tick, counter 1 = refresh, counter 2 = speaker | 5150 boundary (ai_hw_engineer) |
| 060h–063h | R/W | 8255 PPI + keyboard | scan codes, speaker gate, equipment flags | 5160 Tech Ref |
| 0A0h | — | NMI mask | enable/disable NMI (parity) | 5160 Tech Ref |
| 1F0h–1F7h | — | (reserved) | no IDE on the XT; fixed disk is Xebec at 320h | XT spec XT2 |
| 278h–27Fh | R/W | LPT2 (if fitted) | printer port | 5160 Tech Ref |
| 320h–323h | R/W | Xebec fixed-disk adapter | six-byte DCB protocol; DMA channel 3, IRQ5 | XT spec §Hardware contract |
| 378h–37Fh | R/W | LPT1 | printer port | 5160 Tech Ref |
| 3D0h–3DFh | R/W | CGA (6845 + mode) | 3D4h/3D5h CRTC index/data, 3D8h mode, 3D9h palette, 3DAh status | 5160 Tech Ref |
| 3F0h–3F7h | R/W | 8272A diskette | two 360 KiB drives (A:, B:) | 5150 boundary + XT spec |
| 3F8h–3FFh | R/W | 8250 COM1 (if fitted) | serial | 5160 Tech Ref |

Interrupts software uses: INT 10h (video), INT 13h (diskette + Xebec fixed
disk), INT 16h (keyboard), INT 1Ah (clock), INT 21h (DOS 2.00 services for
hosted titles). IRQ lines: 0 tick, 1 keyboard, 5 fixed disk, 6 diskette;
others per Tech Ref.

ASSUMPTION entries (behavior the documentation does not state) go here too,
each with how it will be validated. None yet: every row above is Tech-Ref
or measured-oracle behavior.
