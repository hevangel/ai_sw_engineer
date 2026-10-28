# MC6809 ISA contract

Sources: the Motorola MC6809 Programming Manual / data sheet for
semantics and cycle counts, cross-checked instruction-for-instruction
against the ai_hw_engineer `motorola_6809` SystemVerilog RTL (whose
reyco2000 functional and interrupt suites pass). The reyco2000 cycle
suite is the timing authority for this core: its per-instruction checks
pass with zero mismatches. Where this document and the code disagree,
the code is the defect.

## Machine model

- 8-bit A/B/DP/CC; 16-bit X/Y/U/S/PC. CC: C(0) V(2) Z(4) N(5) I(6)
  H(9?) — bits: C=0x01, V=0x02, Z=0x04, N=0x08, I=0x10, H=0x20, F=0x40,
  E=0x80.
- Reset: DP=0, CC = F|I (E clear), registers read as 0, PC from
  $FFFE/$FFFF.
- Memory: flat 64K, one bus access per clock.
- Stacks: pushes decrement then store (S/U end at the last byte);
  16-bit values sit big-endian (high byte at the lower address).

## Addressing

- Indexed postbyte: bit7=0 → 5-bit signed offset (bits 4:0), register in
  bits 6:5 (X/Y/U/S). bit7=1 → type in bits 4:0: ,R+ / ,R++ / ,-R /
  ,--R / ,R / B,R / A,R / n8,R / n16,R / D,R / n8,PCR / n16,PCR; bit4
  set = indirect ([,R++] / [,--R] / [,R] / [B,R] / [A,R] / [n8,R] /
  [n16,R] / [D,R] / [n8,PCR] / [n16,PCR] / [n16] extended-indirect).
- ASSUMPTION (M6809-A1): reserved postbytes decode as offset-0 indexing
  (shared with the RTL; the reyco suite never uses them).
- B,R and A,R offsets are signed; D,R is unsigned; PCR bases are the
  post-instruction PC.
- ASSUMPTION (M6809-A2): mixed-class TFR/EXG zero-extends 8→16 and
  truncates 16→8 (data-sheet-undefined; matches the RTL).

## Flags

- Subtracts: C is the borrow. V per (x^y)&(x^r) for subtracts and
  (x^r)&(y^r) for adds; H from the low nibbles of adds only.
- Stores: N/Z from the stored value, V cleared (STA/STB 8-bit; STD/
  STX/STY/STU/STS 16-bit).
- MUL: Z from the 16-bit product, C = product bit 7. ABX adds B unsigned.
- DAA: the 6800-family table; V unchanged (ASSUMPTION M6809-A3, matches
  the RTL).
- 6809 shift V rules (suite-derived, shared with the RTL): LSR and ROR
  clear V; ASL/ASR/ROL set V = MSB(result) xor old MSB; NEG V = val
  0x80; COM V = 0; DEC V = val 0x80; INC V = val 0x7F; TST preserves C.

## Interrupts and control

- Vectors: $FFF2 SWI3, $FFF4 SWI2, $FFF6 FIRQ, $FFF8 IRQ, $FFFA SWI,
  $FFFC NMI, $FFFE RESET.
- Priority NMI > FIRQ > IRQ. IRQ/FIRQ level-sampled, NMI edge-latched.
- ASSUMPTION (M6809-A4): NMI edges latch even while NMI is disarmed and
  fire once LDS or LEAS writes S (the data sheet's "NMI disabled until
  the stack pointer is loaded", implemented like the RTL).
- IRQ/NMI push the entire frame (12 bytes, stacked CC with E set); FIRQ
  pushes CC (E clear) + PC only. Live CC after the frame: E mirrors the
  frame type; FIRQ sets F and I; IRQ sets I; SWI sets I and F;
  SWI2/SWI3 and NMI leave the masks unchanged.
- CWAI: CC ← CC AND imm, push the entire frame (stacked CC with E set),
  wait; the waking interrupt vectors without pushing again and sets live
  E (plus its flavor's mask updates).
- SYNC: stop; an unmasked pending interrupt is serviced with a full
  frame, a masked pending source (or a disarmed NMI) resumes execution.
- Response timing: FIRQ 10, IRQ/NMI 16, SWI 19, SWI2/SWI3 20 clocks;
  RTI 6 (E clear) / 15 (E set).

## Timing

Instruction cycle totals follow the data sheet and the reyco2000 cycle
suite (which verifies every measured form with zero mismatches):
inherent 2, ABX 3, MUL 11, TFR 6, EXG 8, short branches 3, long branches
5/6 (LBRA 5 always), PSHS/PULS/PSHU/PULU 5 + bytes, RTS 5, JSR 7/8/12 by
mode, indexed base 4 (8-bit load), 5 (8-bit store, 16-bit load), 6
(RMW, 16-bit store, 16-bit ALU), 7 (16-bit compare) plus the postbyte
overhead, indirect overheads per the suite's table.

Known RTL timing deviations (documented, not reproduced here): the
ai_hw_engineer RTL is 1-2 cycles short on several direct/extended/
indexed forms and times out on the reyco2000 cycle suite's SYNC check —
see `report/final_report.md`.
