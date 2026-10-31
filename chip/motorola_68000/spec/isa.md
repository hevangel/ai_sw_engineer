# MC68000 — ISA notes for the Rust core

Scope: the subset of the MC68000 instruction set the core implements,
with the deviations the SingleStepTests 680x0 oracle requires. The
oracle is authoritative: where this document and the Motorola PRM
disagree, the captured vectors win. Full suite-derived detail lives in
`emulator/spec/singlesteptests.md`.

## Registers and state

- D0-D7, A0-A6, USP/SSP banking on S, PC.
- SR: T/S/I2-0 in `sri`; X/N/Z/V/C in split latches (`fc`, `fx`, `fv`,
  `fnz`). Z is stored inverted (not-zero bit) so ABCD/SBCD/ADDX/SUBX
  can keep Z untouched on a zero result.

## Instruction groups

| Rows | Instruction | Suite notes |
| --- | --- | --- |
| 0000 ss=00-10 | ORI/ANDI/SUBI/ADDI/EORI/CMPI | immediate; CCR/SR forms at …3C/…7C |
| 0000 08xx | static bit ops | kind = bits 7:6 (0 BTST, 1 BCHG, **2 BCLR, 3 BSET**) |
| 0000 rrr1 ss≠01 | dynamic bit ops | same kind map; EA modes 5/6/7 allowed |
| 0000 rrr1 ss=01 | MOVEP | An = bits 2-0, dir = bit 7, size = bit 6 |
| 0040-0043 | NEGX/CLR + SR/CHK | CHK hoisted (all Dn) |
| 0044-0047 | NEG/NOT + CCR/SR | MOVE SR,<ea> swaps modes 3/4 |
| 0048 | NBCD/SWAP/EXT/PEA/MOVEM | NBCD suite formula |
| 004E/4F | LINK/UNLK/TRAP/…/JSR/JMP/LEA | LEA includes row 4F |
| 0xxx-3xxx | MOVE.b/w/l | |
| 5xxx | ADDQ/SUBQ/Scc/DBcc | DBcc branches from the disp word |
| 6xxx | Bcc/BSR | BSR odd target: 18-byte push |
| 8xxx/Cxxx | OR/ABC D/MULU/MULS/SBCD/EXG/AND | MUL gate F0C0, V always clear |
| 9xxx/Dxxx | SUB/SUBA/SUBX/DIVS, ADD/ADDA/ADDX | X (not C) is the extend |
| Bxxx | CMP/EOR/CMPA | |
| Exxx | shifts/rotates | memory form is a word RMW |

## Flag quirks the oracle requires

- Bit ops (BTST/BCHG/BSET/BCLR): Z only; N = bit 31 for register
  destinations.
- NBCD/ABCD/SBCD: decimal corrections, suite V windows — see
  `emulator/spec/singlesteptests.md`.
- MULU/MULS: V and C always clear.
- DIVU overflow: N cleared, V set, operands unchanged.
- GT (condition 14): true only when C=V=N=Z=0.

## Exceptions

- Two-word frame (PC, SR) for TRAP/TRAPV/CHK/ILL/privilege/divzero;
  the CHK/TRAP frames push the pc past the EA extension words.
- 14-byte address-error frame: internal-information word
  `(op & 0xFFE0) | SSW`, fault address (4), IIRD, SR, fault pc (4).
  SSW = 0x15 data read / 0x05 data write / 0x1E instruction fetch.
- Word/long accesses at odd addresses fault; pre-decrement faults
  with the register already stepped (the long form steps in two word
  moves).
- An instruction that completes leaving an odd pc faults on the next
  fetch; the suite counts that fault in the same vector.

## Known gaps (replay 898,188 / 1,000,060)

- to-SR family (MOVEtoSR/ORItoSR/EORItoSR): the suite's reference
  memory is non-zero where the vector init list is empty, so the
  loaded SR value cannot be reproduced from the given bytes yet.
- RTS/RTR fault frames: trailing frame bytes after the first word.
- Small residues: CMPA/CMP/MOVEM/DBcc/Scc/DIVU/SBCD/ABCD/MOVEP.
