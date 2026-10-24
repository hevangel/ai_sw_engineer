# Intel 8008 instruction set

Primary source: *Intel MCS-8 Users Manual*, November 1973, "Summary of
Processor Instructions" (scan pages 12–13), transcribed in
`../ai_hw_engineer/design/intel_8008/references/intel_mcs8_users_manual_nov1973.md`.
Cross-check: the opcode and length tables of SIMH `i8008.c` (Hans-Åke Lund,
2017, MIT-style licence) in the same folder. The MC encode tests in
`tests/mc/` check every encoding below against this table.

Intel announced the 8008 in April 1972 (*Electronics*, and the MCS-8 manual's
first edition); the Micral N (R2E, 1973) is the first machine this backend
targets.

## Programmer's model

| Item | Width | Notes |
|---|---|---|
| A | 8 | accumulator; every ALU result lands here |
| B, C, D, E, H, L | 8 | index registers |
| M | 8 | the memory byte at `H[5:0]:L` (14-bit address; H bits 7–6 ignored) |
| PC | 14 | lives in an 8-entry on-chip stack; 7 return levels are usable |
| flags | 4 | carry C, zero Z, sign S, even parity P |

There is **no data stack, no stack pointer and no indirect jump**. Code and
data share one 16 KiB byte address space.

Register field encoding (`DDD`/`SSS`): A=0, B=1, C=2, D=3, E=4, H=5, L=6,
M=7.

## Flags

- ALU group (`AD AC SU SB ND XR OR CP`, register / M / immediate) sets all four
  flags. Carry is the ninth result bit; for `SU`/`SB`/`CP` it is the borrow.
  `ND`, `XR`, `OR` clear carry. `CP` leaves A unchanged.
- `INr`/`DCr` set Z, S, P and leave carry unchanged. They do not exist for A
  (`r ≠ A`) or M.
- The four rotates change only carry.
- Loads (`Lrr`, `LrM`, `LMr`, `LrI`, `LMI`), jumps, calls, returns, `INP`,
  `OUT` leave the flags unchanged.

Condition field `C4C3`: 00 = carry, 01 = zero, 10 = sign, 11 = parity (even).
`F` forms test the flag false, `T` forms test it true.

## Instructions

Original 1972 mnemonics (the ones SCELBAL and the R2E manual use). Octal
opcodes; `d`/`s` are register fields, `c` the condition field.

| Mnemonic | Bytes | Opcode (octal) | Operation | States |
|---|---|---|---|---|
| `Lds` | 1 | `3ds` (d,s ≠ 7) | d ← s | 5 |
| `LdM` | 1 | `3d7` (d ≠ 7) | d ← M | 8 |
| `LMs` | 1 | `37s` (s ≠ 7) | M ← s | 7 |
| `LdI` imm | 2 | `0d6` (d ≠ 7) | d ← imm | 8 |
| `LMI` imm | 2 | `076` | M ← imm | 9 |
| `INd` | 1 | `0d0` (d = 1..6) | d ← d+1 | 5 |
| `DCd` | 1 | `0d1` (d = 1..6) | d ← d−1 | 5 |
| `ADs` `ACs` `SUs` `SBs` `NDs` `XRs` `ORs` `CPs` | 1 | `2os` (s ≠ 7) | A ← A op s | 5 |
| `ADM` … `CPM` | 1 | `2o7` | A ← A op M | 8 |
| `ADI` `ACI` `SUI` `SBI` `NDI` `XRI` `ORI` `CPI` imm | 2 | `0o4` | A ← A op imm | 8 |
| `RLC` `RRC` `RAL` `RAR` | 1 | `002` `012` `022` `032` | rotate A | 5 |
| `JMP` addr | 3 | `104` | PC ← addr | 11 |
| `JFc` / `JTc` addr | 3 | `1c0` / `1(4+c)0` | jump if flag false / true | 9 / 11 |
| `CAL` addr | 3 | `106` | push PC+3; PC ← addr | 11 |
| `CFc` / `CTc` addr | 3 | `1c2` / `1(4+c)2` | call if flag false / true | 9 / 11 |
| `RET` | 1 | `007` | pop PC | 5 |
| `RFc` / `RTc` | 1 | `0c3` / `0(4+c)3` | return if flag false / true | 3 / 5 |
| `RST` n | 1 | `0n5` | push PC+1; PC ← 8·n | 5 |
| `INP` p | 1 | `1(0p)1`, p = 0..7 | A ← input port p | 8 |
| `OUT` p | 1 | `1(p)1`, p = 8..31 | output port p ← A | 6 |
| `HLT` | 1 | `000`, `001`, `377` | halt | 4 |

`o` is the ALU operation: AD=0, AC=1, SU=2, SB=3, ND=4, XR=5, OR=6, CP=7.

Address bytes are little-endian: low byte first, then the high byte, whose bits
7–6 are ignored. `JMP` and `CAL` are also decoded with the "don't care" bits
3–5 set (`1xx4` / `1xx6`); the assembler emits only `104` / `106`.

Opcodes with no defined meaning (`042 052 062 070 071 072`) and the duplicate
`RET` and `JMP`/`CAL` encodings are never emitted. The disassembler prints the
undefined ones as `.byte`.

Encoding of `INP`/`OUT`: `01 RR MMM 1`. `INP` uses RR = 00 (ports 0–7); `OUT`
needs RR ≠ 00 (ports 8–31). The port number is `RRMMM`.

## What the Micral N adds

On the Micral N, the 8008's I/O cycle puts A on the low address byte and the
port number on address bits 9–13. The I/O cards decode both, so `INP p` reads
from a card selected by `p` **and by the value in A** (the card uses A's low
bits as a sub-address). The platform docs (`system/micral_n/platform/io_map.md`)
describe those conventions. The instruction encodings themselves are unchanged.
