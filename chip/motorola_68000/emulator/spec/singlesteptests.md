# SingleStepTests 680x0 — suite-derived semantics

Notes captured while fitting the MC68000 core to the SingleStepTests
680x0 vector suite (1,000,060 vectors replayed by
`tests/vector_replay.rs`; per-row probe in `tests/probe.rs`, env
`PROBE_FILE`, `PROBE_ROW`, `M68K_VECTOR_DIR`).

The suite's file names do **not** describe the vectors inside a file:
files sweep opcode families whose real decodes spread across several
instructions (the "MOVEP.w" file carries dynamic BSET rows, the "BCLR"
file carries BTST/BCHG/BSET-shaped rows, "CHK" carries every
destination register, and so on). Every rule below was fitted from the
captured expected state, not from the file names or the canonical
manual.

## Current replay state

970,952 / 1,000,060 passing (97.1%). Remaining classes: the
ADD/ADDX/SUBX/OR families (~13,400 rows, unswept golden quirks),
MOVEM.w/l residue (~1,860: pre-dec store slot order and fault
corners), MOVE.l / LINK (~2,980), ABCD/NBCD (~950), Bcc/DBcc (209).

## Fitted this round (session 3)

- **DIVU/DIVS word forms** (both files now fully pass): remainder in
  the high word, quotient in the low word of Dq (bits 11-9); N = bit
  15 of the quotient, Z = quotient word zero, V forced clear, C
  cleared, X preserved. Overflow (quotient out of 16 bits, or
  $80000000/−1 for DIVS) leaves the operands and N/Z/X untouched with
  V set and C cleared. Divide-by-zero stacks the **instruction
  address** (not pc+2) and SR with **N, Z and C cleared** (X, V
  preserved); only one memory-operand divzero row exists per DIV
  file, and register-divisor divzero rows don't occur at all.
- The suite's "divzero-looking" rows at handler 0x1400 are actually
  **address-error rows** (out-of-range or odd EAs) — distinguish by
  the actual zero divisor read in the transaction list, not by the
  handler address.
- **MOVE <ea>,SR decode**: the arm must test bits 7:6 = 11
  (0x46C0-FF), not bits 8:6 = 111 — the latter never matches and the
  rows fall into NOT. The golden's loaded SR = source & 0xA71F (T, S,
  bits 10:8, CCR kept) — set_sr's implicit mask already does this.
- **Trace samples T at instruction start**: an instruction that
  loads T=1 (ORI/EORI/MOVE to SR, RTE) completes without tracing.
- **Exception entry keeps the interrupt mask** (S forced, T cleared,
  I untouched) — visible only on RTE format faults with a popped
  I-field below 7.
- **RTE to an odd address**: the frame's internal word is 0x1E while
  the popped SR keeps supervisor mode and 0x1A once the RTE would
  have dropped to user mode; the fault address is the popped PC and
  the register steps commit before the fault.
- **MOVE SR,<ea>** uses canonical (An)+/-(An) stepping (write at the
  original An, register steps by 2 even on the faulting write).
- **CMPM gate = opmode 5/6 with bits 5:3 = 001**: opmode 7 with bits
  5:3 = 001 is CMPA.L with an address-register-direct operand. CMPM
  reads go through odd_ea so an odd (An)+ faults with the step
  committed; the golden's CMPM fault rows read the bits-11:9 register
  first when the bits-5:3 register is even.
- **CMP/CMPA never touch X** (do_sub_x with update_x = false) and
  never write the destination register.
- **MOVEM**: word loads replace the whole destination register with
  the sign-extended value; the pre-dec forms step by the transfer
  size (pass Word/Long, not a fixed Long, to the EA helper) and roll
  the register back on a parity fault (store faults at reg-2 with the
  write label, load at the block base with the read label); the
  (An)+ load form ends the register at orig + step x popcount.

## Fitted this round (session 2)

- **Immediate EA (mode 7 reg 4)**: the fetched immediate IS the
  operand — earlier code read memory at its value. RMW/write forms of
  the immediate EA take the illegal exception.
- **Bcc/BSR 16-bit displacement base** = the extension word's own
  address (opcode + 2), confirmed against GNU objdump.
- **BSR to odd target**: the return-address push (0xC02) commits at
  [7FC-FF] BEFORE the frame; the standard 14-byte frame then goes at
  [7EE-7FB] (SSW = `(op&0xFFE0)|0x1E`, addr = target, IIRD = opcode,
  SR, fault pc = target−4); ssp ends at 0x7EE.
- **Post-instruction fetch faults** (RTS/RTR to an odd popped pc): the
  replay harness takes a second step; the frame is the standard
  14-byte frame at the POST-POP stack pointer, fault pc = pc−4,
  IIRD = the completed opcode.
- **TRAP #n**: the suite preloads vectors at **0x80 + 4n** (not 32)
  and sweeps the whole 4E40-4E4F row (mask 0xFFF0).
- **CMPM** steps both (An)+ registers by the operand size.
- **Bit ops** (BTST/BCHG/BSET/BCLR): Z only — N, C, V, X untouched
  for both register and memory destinations (earlier "N = bit 31"
  fit was wrong).

## Decode rules the suite actually uses

- **MOVEP** = `0000 rrr 1 dir sz 0 0 1 An`: bit 8 set, mode bits 5:3 =
  001, An = bits 2-0, direction bit 7 (1 = Dn to memory), size bit 6.
  Displacement word always fetched; no flags, no register stepping.
- **Dynamic bit ops** = every other bit-8 row: kind from bits 7:6 with
  the suite's swap — 0 BTST, 1 BCHG, **2 BCLR, 3 BSET** — bit register
  in bits 11-9, EA in bits 5:0. The suite allows EA modes 5/6/7 for
  the dynamic form.
- **Static bit ops** = 0x08xx only (any bits 7:6), immediate byte at
  pc+2, same kind map.
- Bit ops touch **Z only** (Z clears on a set bit, keeps its value on
  a clear bit); N follows bit 31 for register destinations and is
  untouched for memory destinations; C/V/X are never written.
- **CHK.W** = rows 4180-4FFF odd with ss = 10: every destination
  register in bits 11-9 (not just D0). The CHK exception pushes the pc
  **after** the EA extension words. Out-of-bounds sets N per the sign
  of Dn, clears Z.
- **MULU/MULS** = `op & 0xF0C0 == 0xC0C0` (any destination register).
  N/Z are canonical; **V is always cleared**, C always cleared.
- **ADDQ/SUBQ** require bit 8 set; **DBcc** is `op & 0x00F8 == 0x00C8`
  (any condition — bit 8 is the condition's low bit); Scc takes all
  remaining `ss = 11` rows.
- **DBcc** branches relative to the **displacement word's own
  address** (instr+2), not the canonical instr+4. A faulting branch
  target pushes that same address as the frame PC.
- **GT (cc 14)** is true only when **C, V, N and Z are all clear**
  (fitted from thousands of DBcc rows; canonical GT disagrees at
  C=0/V=1/N=1/Z=0 and friends). Every other condition is canonical.

## BCD arithmetic (NBCD / ABCD / SBCD)

- **NBCD**: `res = (0 - dst - X + corr) & 0xFF` with corr = 0 when
  dst+X = 0, else 0x9A when the low nibble of dst+X is nonzero, else
  0xA0. C/X = (dst+X != 0); Z clears only on a nonzero result; N =
  bit 7 of the result; V is set exactly for `0x1B <= dst+X <= 0x80`.
- **ABCD / SBCD**: nibble-serial decimal arithmetic — a nibble sum
  above 9 (ABC) or below 0 (SBC) takes a ±6 correction and its carry
  (which can exceed one for invalid digits) feeds the next nibble.
  C/X = the high-nibble carry/borrow; Z clears only on a nonzero
  result; N = bit 7 of the result. V windows (empirical):
  - ABCD: c1 set → `res & 0xC0 == 0x80`; c0 only → `0x80 <= t+6 <=
    0x85` (t = raw byte sum); else false.
  - SBCD: with u = t - 6*b0: (b0,b1) = (0,1) → `u in [-128,-34]`;
    (1,1) → `u in [-132,-33]`; (1,0) → `u in [123,126]`; (0,0) →
    false.

## Faults and frames

- The suite labels **every data fault SSW = 0x15** (read) — write
  faults included — except where a pure write is distinguished.
- **Pre-decrement word/long accesses fault on an odd post-decrement
  address with the register already stepped** (the long form decrements
  in two word steps, so an odd base faults after the first -2).
- `ea_write_checked` computes the EA once; calling odd_ea and write_ea
  both stepping (An)+/-（An) double-steps the register.
- **The harness runs a second step** when one step leaves an odd pc:
  the RTS "returns" to an odd address, and the *following* fetch fault
  is part of the same vector (suite ssp drops by 18 = the RTS's +4 pop
  then a 14-byte fault frame).
- **IIRD of a fetch fault = the last bus-latched word**: the opcode of
  the interrupted instruction, or the high word a completing RTS/RTR
  popped (`iird` field). In-instruction data faults push the opcode.
- **BSR to an odd target**: the 14-byte frame is followed by a 4-byte
  continuation push of `(op & 0xFFE0) | 0x1E` in the high half (the
  resumed return-address push); ssp drops by 18.
- `compare_memory` (test harness) reports **all** memory diffs, not
  just the first — single-diff reporting hid frame details for hours.

## Fitting workflow that worked

1. `PROBE_FILE=<name> PROBE_MAX=0` dumps every failing row of one file
   to `%TEMP%/m68k_probe.csv` (columns include the full initial state,
   suite-final registers and a memory-diff string).
2. Register-form rows give (input → expected) pairs for formula
   fitting (NBCD dst sweep, SBCD/ABCD src×dst grids, flag maps).
3. `PROBE_ROW=<n>` prints the complete init/want/got of one row —
   needed whenever a memdiff byte could belong to several frame
   fields.
4. The memdiff string reports **every** differing byte (fixed this
   session); before that it stopped at the first, which hid entire
   frame layouts.
