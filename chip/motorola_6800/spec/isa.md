# MC6800 ISA contract

Source of truth: Motorola M6800 Programming Reference Manual (M68PRM(D),
Nov 1976), cross-checked instruction-for-instruction against the project's
independent instruction-set oracle
(`ai_hw_engineer/design/motorola_6800/scripts/make_isa_vectors.py`) and the
`motorola_6800` SystemVerilog RTL. The full-ISA vector replay pins this core
to the oracle at every opcode-fetch boundary, so where this document and the
code disagree, the code is the defect.

## Machine model

- 8-bit ACCA/ACCB, 16-bit IX/SP/PC, 8-bit CC: C(0) V(1) Z(2) N(3) I(4)
  H(5); bits 7-6 always read as 1.
- Reset: PC from $FFFE/$FFFF; CC = $D0 (bits 7-6 set, I set).
  ASSUMPTION (M6800-A1): A, B, IX, SP read as 0 after reset. Real silicon
  leaves them undefined; the project oracle and RTL define a deterministic
  zero state, which this core shares.
- Memory: flat 64K, byte-wide bus. The core drives one bus access per
  clock; every instruction's cycle count matches the M68PRM execution-time
  table (pads fill interior cycles the PRM does not assign bus activity
  to; pad placement is an approximation, totals are exact).
- Stack: push writes at SP then decrements; pull increments then reads.

## Interrupts

- IRQ is level-sampled, NMI edge-latched (single-clock pulse latches one
  request), both acted on at instruction boundaries. A preempted fetch
  never happens (no boundary record, no fetch bus cycle).
- Service: 12 clocks — 7 register pushes (PCL, PCH, IXL, IXH, A, B,
  CC|bits7-6), I set, vector fetch. NMI has priority over IRQ; NMI is
  cleared when taken, IRQ is level (stays until deasserted).
- SWI: unconditional, pushes, sets I, vectors $FFFA.
- WAI (9 clocks): pushes the return address after WAI, then waits; wake
  on NMI or unmasked IRQ sets I and vectors without pushing again.

## Semantics carried from the PRM via the project oracle

- Subtract carry is the borrow (C set if |M| > |ACCX|).
- DAA per M68PRM: (N,Z from result; C from the high-nibble correction).
- DEC/INC leave C unchanged; INX/DEX/INS/DES/TSX/TXS update only Z (or
  nothing); CBA updates C, V, N, Z from ACCB-ACCA without writing a
  result.
- STA sets no flags in this project's model (see A3 below). STS/STX set
  N/Z from the stored 16-bit value and clear V.

## Assumption ledger

- ASSUMPTION (M6800-A2): unassigned opcodes execute as NOPs of the same
  length the oracle assigns (1-byte, or operand-consuming 2/3-byte forms),
  matching the RTL's documented assumption. M68PRM documents 197 valid
  codes; behavior of the other 59 is undefined on real silicon.
- ASSUMPTION (M6800-A3): STA does not update N/Z/V. M68PRM Table 2-1 says
  STA sets N, Z and clears V, but the project oracle and RTL both leave
  CC untouched on the STA store cycle (RTL `motorola_6800.sv`:
  `S_WR: state <= S_FETCH;` with no CC update). This core follows the
  project contract. Flagged as a potential PRM deviation for
  [ai_engineer#43](https://github.com/hevangel/ai_engineer/issues/43);
  if the RTL ever adopts PRM semantics, this core and `spec/isa.md`
  change in the same commit.
- ASSUMPTION (M6800-A4): the $E000 control port (0 = IRQ low, 1 = IRQ
  high, 2 = NMI pulse, $AA = done) is harness-side, not part of the CPU;
  the core only performs the bus writes and exposes interrupt line
  inputs.
