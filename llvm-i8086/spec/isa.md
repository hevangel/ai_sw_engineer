# i8086 instruction set

Subset the backend emits: the original 8086/8088 instruction set. Source: Intel, *The 8086 Family User's Manual*, October 1979, order number 9800722-03 (the copy in the hardware project's `references/intel_8086_family_users_manual_1979.pdf`, SHA-256 `717194ec03520d6499a40523abdfd4ed15a2b8ccc45a21c653e2b443c248664c`; it is copyrighted and not in this repository). The page of every mnemonic is in the index below; the encodings were then verified against the physical chip, which is the stronger oracle (see Verification status).

## Manual page index

Pages are the manual's own numbers (chapter 2, "8086 and 8088 Central Processing Units"). *Description* is the instruction's entry in section 2.4 (pages 2-31 to 2-48); *Reference* is its entry in Table 2-21, "Instruction Set Reference Data" (pages 2-51 to 2-68: the encoding bytes, clock counts and flag effects per operand form). The index was made by reading the manual's pages, not by trusting the backend's own table.

| Mnemonic | Description | Reference |
|---|---|---|
| AAA | 2-35 | 2-51 |
| AAD, AAM | 2-37 | 2-51 |
| AAS | 2-36 | 2-51 |
| ADC, ADD | 2-35 | 2-52 |
| AND | 2-38 | 2-52 |
| CALL | 2-43 | 2-52 |
| CBW | 2-38 | 2-52 |
| CLC, CLD, CMC | 2-47 | 2-53 |
| CLI | 2-48 | 2-53 |
| CMP | 2-36 | 2-53 |
| CMPSB, CMPSW | 2-42 | 2-53 |
| CWD | 2-38 | 2-54 |
| DAA, DAS, DEC | 2-36 | 2-54 |
| DIV | 2-37 | 2-54 |
| HLT | 2-48 | 2-55 |
| IDIV, IMUL | 2-37 | 2-55 |
| IN | 2-32 | 2-55 |
| INC | 2-35 | 2-55 |
| INT, INT3 | 2-46 | 2-56 |
| INTO, IRET | 2-47 | 2-56 |
| JA/JNBE, JAE/JNB, JB/JNAE | 2-46 (table of conditions) | 2-56 |
| JBE/JNA, JC, JCXZ, JE/JZ, JG/JNLE, JGE/JNL, JL/JNGE | 2-46 | 2-57 |
| JLE/JNG, JNC, JNE/JNZ, JNO, JNP/JPO, JNS | 2-46 | 2-58 |
| JO, JP/JPE, JS | 2-46 | 2-59 |
| JMP | 2-45 | 2-58 |
| LAHF | 2-32 | 2-59 |
| LDS, LEA, LES | 2-32 | 2-59 |
| LOCK | 2-48 | 2-60 |
| LODSB, LODSW | 2-43 | 2-60 |
| LOOP, LOOPE/LOOPZ | 2-45 | 2-60 |
| LOOPNE/LOOPNZ | 2-46 | 2-60 |
| MOV, POP, PUSH | 2-31 | 2-61, 2-62, 2-63 |
| MOVSB, MOVSW | 2-42 | 2-61 |
| MUL | 2-36 | 2-61 |
| NEG | 2-36 | 2-62 |
| NOP | 2-48 | 2-62 |
| NOT, OR | 2-38 | 2-62 |
| OUT | 2-32 | 2-62 |
| POPF, PUSHF | 2-33 | 2-63 |
| RCL, RCR | 2-40 | 2-63 |
| REP, REPE/REPZ | 2-42 | 2-63 |
| REPNE/REPNZ | 2-42 | 2-64 |
| RET, RETF | 2-45 | 2-64 |
| ROL | 2-39 | 2-64 |
| ROR | 2-40 | 2-64 |
| SAHF | 2-33 | 2-64 |
| SAL/SHL, SAR | 2-39 | 2-65 |
| SBB | 2-36 | 2-65 |
| SCASB, SCASW | 2-43 | 2-65 |
| SHR | 2-39 | 2-66 |
| STC, STD | 2-47 | 2-66 |
| STI | 2-48 | 2-66 |
| STOSB, STOSW | 2-43 | 2-66 |
| SUB | 2-36 | 2-67 |
| TEST | 2-39 | 2-67 |
| WAIT | 2-48 | 2-67 |
| XCHG, XLAT | 2-32 | 2-67 |
| XOR | 2-38 | 2-68 |

`ESC` (2-48), the 80186+ opcodes and the undocumented ones are rejected on purpose.

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
