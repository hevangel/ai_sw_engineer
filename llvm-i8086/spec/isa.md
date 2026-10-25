# i8086 instruction set

Subset the backend emits: the original 8086/8088 instruction set. Source: Intel, *The 8086 Family User's Manual* (edition to be confirmed) instruction-set chapter and opcode map; **page numbers and a row-by-row check against the manual are owed** and are the first job of milestone 2 (each row below becomes one MC encode test in `tests/mc/`, checked against the manual's opcode table, per AGENTS). Until then this document is a design summary, not a verified reference.

## Registers

| Name | Width | Role |
|---|---|---|
| AX BX CX DX | 16 (AL/AH, BL/BH, CL/CH, DL/DH as bytes) | general; AX accumulator, CX count/shift, DX I/O port and high half of 32-bit results |
| SI DI | 16 | index; string source and destination |
| BP | 16 | base pointer (default segment SS) |
| SP | 16 | stack pointer (SS) |
| CS DS ES SS | 16 | segment registers |
| IP, FLAGS | 16 | instruction pointer; CF PF AF ZF SF TF IF DF OF |

## Verification status (2026-10-25)

The encoding table below is no longer only a design summary. It is implemented as TSFlags in `target/lib/Target/I8086/I8086InstrFormats.td` and checked three ways (`tests/`): 423 rows written from the manual's opcode structure (`mc/encodings.s`), 36 branch encodings with their fixups, and `mc/vector_check.py`, which decodes and re-assembles tens of thousands of instruction instances captured from a physical Intel P80C86A-2 (SingleStepTests/8086), every opcode form, with MartyPC's mnemonic as the cross-check. Page numbers into Intel's manual are still to be added to the rows; until they are, the corpus is the authority for what the chip does with each byte string.

## Encoding summary

ModRM byte: `mod(2) reg(3) rm(3)`. `mod=11` selects a register; otherwise `rm` selects `[BX+SI] [BX+DI] [BP+SI] [BP+DI] [SI] [DI] [BP] [BX]`, with `mod=00 rm=110` meaning a direct 16-bit address and `mod=01/10` adding an 8-bit sign-extended or 16-bit displacement. Bases using BP default to the SS segment, others to DS. Segment override prefixes: 26h ES, 2Eh CS, 36h SS, 3Eh DS. Opcode low bit `w` selects byte (0) or word (1); bit 1 `d` selects the direction for ModRM ALU/MOV forms.

| Group | Opcodes (hex) | Notes |
|---|---|---|
| MOV | 88/89 r/m,r; 8A/8B r,r/m; 8C/8E sreg; B0+r/B8+r r,imm; C6/C7 r/m,imm; A0-A3 AL/AX with direct address | |
| PUSH/POP | 50+r / 58+r; FF /6, 8F /0; 06 0E 16 1E / 07 17 1F segment forms; 9C PUSHF, 9D POPF | `0F` is `POP CS` on the 8086: never emitted |
| ALU (ADD OR ADC SBB AND SUB XOR CMP) | 00-3D in blocks of 8 (r/m,r; r,r/m; AL/AX,imm); 80 /n r/m8,imm8; 81 /n r/m16,imm16; 83 /n r/m16,imm8 sign-extended | /0 ADD /1 OR /2 ADC /3 SBB /4 AND /5 SUB /6 XOR /7 CMP |
| INC/DEC | 40+r / 48+r (16-bit); FE/FF /0 /1 | do not change CF |
| NOT NEG MUL IMUL DIV IDIV TEST | F6/F7 /2 /3 /4 /5 /6 /7 /0; 84/85, A8/A9 | MUL/IMUL give 16x16 to DX:AX; DIV/IDIV take DX:AX / r/m16 |
| Shifts and rotates | D0/D1 by 1; D2/D3 by CL; /0 ROL /1 ROR /2 RCL /3 RCR /4 SHL /5 SHR /7 SAR | the 8086 has **no immediate-count form** (C0/C1 are 186+) |
| Branches | 70-7F Jcc rel8; EB JMP rel8; E9 JMP rel16; E8 CALL rel16; FF /4 /2 indirect JMP/CALL; C3 RET; C2 RET imm16; E2 LOOP; E3 JCXZ | **conditional jumps reach only +/-127 bytes**: the assembler relaxes an out-of-range Jcc to an inverted Jcc over a JMP |
| Condition codes | 70 JO, 71 JNO, 72 JB, 73 JAE, 74 JE, 75 JNE, 76 JBE, 77 JA, 78 JS, 79 JNS, 7A JP, 7B JNP, 7C JL, 7D JGE, 7E JLE, 7F JG | |
| Data movement | 8D LEA; 86/87 XCHG; 90+r XCHG AX,r (90 = NOP); 98 CBW; 99 CWD; 9E SAHF; 9F LAHF | |
| String | A4/A5 MOVS, A6/A7 CMPS, AA/AB STOS, AC/AD LODS, AE/AF SCAS with F3 REP / F2 REPNE prefix | direction by DF; memory copy and fill map onto `REP MOVS` / `REP STOS` |
| I/O and control | E4-E7 and EC-EF IN/OUT; CD INT; CF IRET; F4 HLT; FA CLI; FB STI; FC CLD; FD STD | |

## Excluded (80186 and later, not valid on an 8088)

`PUSH imm` (68, 6A), `PUSHA`/`POPA` (60, 61), `BOUND` (62), `IMUL r,r/m,imm` (69, 6B), `INS`/`OUTS` (6C-6F), shifts and rotates by immediate (C0, C1), `ENTER`/`LEAVE` (C8, C9), and every `0F`-prefixed opcode. The assembler rejects them; the code generator must never select them. 32-bit registers, 66h/67h prefixes and `MOVZX`/`MOVSX` do not exist.

## Cycle counts

Not tabulated. Original execution-unit timing is not part of the correctness contract (the oracle platform also does not model it); size, not speed, is the budget here. Add counts only if a speed analysis needs them, with the manual page.
