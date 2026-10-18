# Intel 4004 instruction set

The 4004 executes the MCS-4 repertoire on 4-bit data: 16 "basic" machine
instructions (`OPR` = 0x0..0xD), 16 RAM/ROM I/O instructions (`OPR` = 0xE), and
16 accumulator-group instructions (`OPR` = 0xF), two of the last being
undefined. The manual's own count is 45 (16 basic + 15 I/O + 14
accumulator); adding the `WPM` program-memory write documented for 4008/4009
systems gives the commonly cited **46**, which this spec enumerates.

Every row below is the reference for one MC encode test in `tests/mc/`: the
`llvm-mc -show-encoding` byte output must equal the `Encoding` column.

## Conventions

- A **word** W is one 8-bit program byte: `OPR` is the high nibble (fetched at
  phase M1), `OPA` the low nibble (phase M2). Two-word instructions occupy two
  consecutive bytes; the second word is `A2A1` (two nibbles) or immediate data.
- `RRRR` = one of 16 index registers R0–R15. `RRR` = one of eight register
  **pairs** P0–P7; the pair's **even** register (R2n, "`RRR0`") holds the high
  nibble and the odd (R2n+1, "`RRR1`") the low nibble.
- `ACC` is the 4-bit accumulator; `CY` the 1-bit carry. `DDDD` is 4-bit
  immediate data. `C4C3C2C1` is the JCN condition field (§4). `AAAA…` fields
  are address nibbles; `A3` is the high nibble carried in word 1 of JUN/JMS.
- Cycles are **instruction cycles** (8 clock periods each). All instructions
  are 1 cycle except the two-word JCN/FIM/JUN/JMS/ISZ (2 cycles) and the
  one-word exception **FIN** (1 word, 2 cycles).

Sources per section are listed under each table; see §9 for the full source
list with URLs. The authoritative behavioral reference used throughout is the
ai_hw_engineer Intel 4004 design spec `design/intel_4004/spec/spec.md` §5,
itself sourced from the MCS-4 manual and cross-validated against the MAME
MCS-40 core by executing the original Busicom ROMs; its RTL
(`design/intel_4004/src/intel_4004.sv`) and the independent chips emulator
(`external/chips` `src/mcs4/cpu4004.rs`) are the cross-checks.

## 1. Basic instructions (`OPR` = 0x0 … 0xD)

| Mnemonic | Encoding (OPR OPA) | Words | Cycles | Operation | Flags |
|---|---|---|---|---|---|
| NOP | `0000 0000` | 1 | 1 | No operation | — |
| JCN | `0001 C4C3C2C1` + `A2A1` | 2 | 2 | If condition (§4) true: `PC ← {PC[11:8], A2A1}`; else fall through | — |
| FIM | `0010 RRR0` + `DDDD DDDD` | 2 | 2 | Load pair: `R(2n) ← D[7:4]`, `R(2n+1) ← D[3:0]` | — |
| SRC | `0010 RRR1` | 1 | 1 | Send pair `{R(2n), R(2n+1)}` as the 8-bit RAM/ROM address pointer (§5) | — |
| FIN | `0011 RRR0` | 1 | 2 | Fetch ROM byte at `{PC[11:8], R0, R1}` into pair `RRR` (`R(2n) ← byte[7:4]`, `R(2n+1) ← byte[3:0]`) | — |
| JIN | `0011 RRR1` | 1 | 1 | `PC[7:0] ← {R(2n), R(2n+1)}`; `PC[11:8]` unchanged | — |
| JUN | `0100 AAAA` + `A2A1` | 2 | 2 | `PC ← {A3, A2, A1}` (12-bit absolute jump) | — |
| JMS | `0101 AAAA` + `A2A1` | 2 | 2 | Push return address (word1+2), then `PC ← {A3, A2, A1}` (§6) | — |
| INC | `0110 RRRR` | 1 | 1 | `R ← (R + 1) mod 16` | CY unaffected |
| ISZ | `0111 RRRR` + `A2A1` | 2 | 2 | `R ← (R + 1) mod 16`; if `R = 0` fall through, else `PC[7:0] ← {A2A1}` (§3) | CY unaffected |
| ADD | `1000 RRRR` | 1 | 1 | `{CY, ACC} ← ACC + R + CY` | CY = carry-out |
| SUB | `1001 RRRR` | 1 | 1 | `{CY, ACC} ← ACC + ~R + ~CY` (§7) | CY = 1 ⇒ no borrow |
| LD | `1010 RRRR` | 1 | 1 | `ACC ← R` | — |
| XCH | `1011 RRRR` | 1 | 1 | `ACC ↔ R` | — |
| BBL | `1100 DDDD` | 1 | 1 | Pop return address into PC (§6); `ACC ← DDDD` | CY unaffected |
| LDM | `1101 DDDD` | 1 | 1 | `ACC ← DDDD` | — |

Source: ai_hw_engineer spec §5.1; opcode matrix and the 1-word/2-cycle FIN
exception cross-checked against the pastraiser opcode table and e4004 iset.

## 2. RAM and ROM port instructions (`OPR` = 0xE)

All operate through the pointer latched by the most recent `SRC`, with the
`CM-RAM` bank lines selected by the `DCL` command register. Write ops drive
`ACC` onto the bus during X2/X3; read ops sample the bus at the end of X3.

| Mnemonic | Encoding | Operation | Flags |
|---|---|---|---|
| WRM | `1110 0000` | RAM main character ← ACC | — |
| WMP | `1110 0001` | RAM output port ← ACC | — |
| WRR | `1110 0010` | ROM I/O port ← ACC (ROM-port context, not `CM-RAM`) | — |
| WPM | `1110 0011` | Program-memory half-byte ← ACC (4008/4009 program RAM; `CM-ROM` asserted at X2/X3) | — |
| WR0 | `1110 0100` | RAM status char 0 ← ACC | — |
| WR1 | `1110 0101` | RAM status char 1 ← ACC | — |
| WR2 | `1110 0110` | RAM status char 2 ← ACC | — |
| WR3 | `1110 0111` | RAM status char 3 ← ACC | — |
| SBM | `1110 1000` | `{CY, ACC} ← ACC + ~(RAM char) + ~CY` (§7) | CY = 1 ⇒ no borrow |
| RDM | `1110 1001` | ACC ← RAM main character | — |
| RDR | `1110 1010` | ACC ← ROM I/O port | — |
| ADM | `1110 1011` | `{CY, ACC} ← ACC + (RAM char) + CY` | CY = carry-out |
| RD0 | `1110 1100` | ACC ← RAM status char 0 | — |
| RD1 | `1110 1101` | ACC ← RAM status char 1 | — |
| RD2 | `1110 1110` | ACC ← RAM status char 2 | — |
| RD3 | `1110 1111` | ACC ← RAM status char 3 | — |

Source: ai_hw_engineer spec §5.2; subtract-carry sense and WPM bus behavior
cross-validated against MAME MCS-40.

## 3. Accumulator-group instructions (`OPR` = 0xF)

| Mnemonic | Encoding | Operation | Flags |
|---|---|---|---|
| CLB | `1111 0000` | `ACC ← 0`, `CY ← 0` | sets both |
| CLC | `1111 0001` | `CY ← 0` | CY ← 0 |
| IAC | `1111 0010` | `{CY, ACC} ← ACC + 1` | CY = carry-out |
| CMC | `1111 0011` | `CY ← ~CY` | complements CY |
| CMA | `1111 0100` | `ACC ← ~ACC` (ones' complement) | — |
| RAL | `1111 0101` | Rotate left through carry: `{CY, ACC} ← {ACC[3:0], CY}` | CY ← old ACC[3] |
| RAR | `1111 0110` | Rotate right through carry: `{ACC, CY} ← {CY, ACC[3:0]}` | CY ← old ACC[0] |
| TCC | `1111 0111` | `ACC ← {0,0,0,CY}`, `CY ← 0` | CY ← 0 |
| DAC | `1111 1000` | `{CY, ACC} ← ACC + 0xF` (i.e. ACC − 1) | CY = 0 ⇒ borrow |
| TCS | `1111 1001` | `ACC ← 9 + CY` (CY=0→9, CY=1→10), `CY ← 0` | CY ← 0 |
| STC | `1111 1010` | `CY ← 1` | CY ← 1 |
| DAA | `1111 1011` | Decimal adjust: if `CY = 1` or `ACC > 9` then `ACC ← (ACC + 6) mod 16`; `CY` set on overflow, else retained | CY set on carry-out |
| KBP | `1111 1100` | Keyboard-process one-hot→binary: 0→0, 1→1, 2→2, 4→3, 8→4, else→0xF | — |
| DCL | `1111 1101` | `CMD ← ACC[2:0]` (select RAM bank) | — |
| — | `1111 1110` | Undefined → executes as NOP (§8) | — |
| — | `1111 1111` | Undefined → executes as NOP (§8) | — |

Source: ai_hw_engineer spec §5.3; TCS/DAA/KBP decimal semantics cross-validated
against MAME MCS-40 running the original Busicom firmware.

**Instruction total:** 14 basic + SRC/FIN/JIN already in §1 (16 rows in §1),
16 in §2, and 14 defined in §3 = **46 defined instructions**, matching the
MCS-4 repertoire (45) plus WPM.

## 4. JCN condition code (`C4C3C2C1` = OPA bits)

| Bit (OPA) | Name | Contribution |
|---|---|---|
| OPA bit 3 (C1) | invert | Invert the OR of the selected conditions |
| OPA bit 2 (C2) | ACC=0 | Selected if set: condition is "ACC = 0" |
| OPA bit 1 (C3) | CY=1 | Selected if set: condition is "CY = 1" |
| OPA bit 0 (C4) | TEST | Selected if set: condition is "TEST pin = 0" |

`raw = (ACC=0)·C2 + (CY=1)·C3 + (TEST=0)·C4`; `jump = C1 ? ~raw : raw`.
With no condition selected `raw = 0`, so `0001 0000` never jumps and
`0001 1000` jumps unconditionally. The target page is `PC[11:8]` **after both
words have been fetched** (see §3 end-of-page note).

Source: ai_hw_engineer spec §5.4 (JCN bit order cross-checked against MAME
MCS-40 and the e4004/pyntel4004 transcriptions).

## 5. SRC pointer and 4001/4002 addressing

`SRC` latches the 8-bit value of the designated register pair as the memory
pointer, used by every `OPR = 0xE` instruction and interpreted differently per
device:

- **4002 RAM** (`WRM`/`RDM`, `WR0-3`/`RD0-3`, `WMP`, `SBM`/`ADM`): pointer
  `[7:6]` selects one of four **RAM chips** in the active bank, `[5:4]`
  selects one of that chip's **four registers**, `[3:0]` selects one of the
  register's **16 main characters**. Each register also has **four status
  characters** addressed *entirely by the opcode* (`WR0`/`RD0` … `WR3`/`RD3`),
  not by the character field.
- **4001 ROM / 4002 output port** (`WRR`/`RDR` ROM I/O ports, `WMP` RAM output
  port): pointer `[7:4]` selects the ROM chip (or 4002 for `WMP`); the low
  nibble is ignored by the port.
- **DCL** (`1111 1101`) loads `CMD ← ACC[2:0]`, decoded onto the four
  `CM-RAM` bank lines so one of up to four 4002 banks is active; `SRC` then
  addresses within that bank. Reset selects bank 0.

A 4002 RAM chip therefore holds 4 registers × 16 main characters = 64 main
nibbles + 4 registers × 4 status characters = 16 status nibbles.

Source: ai_hw_engineer spec §4.2, §5.2, §6.3 (SRC/DCL command-line operation
from the MCS-4 manual).

## 6. Program counter stack (3-level)

The PC lives in a **4-register circular push-down stack** `stack[0..3]` with a
2-bit pointer `SP`; `stack[SP]` is the live PC (12 bits = {ROM chip [11:8],
ROM word [7:0]}, addressing 4096 bytes).

- **JMS**: `SP ← (SP + 1) mod 4`; the return address (first word + 2, i.e. the
  byte after the two-word JMS) is written to the new `stack[SP]`; then
  `PC ← {A3, A2, A1}`. This gives **3 levels of nesting** beyond the running
  routine (4 stack slots total).
- **BBL**: `SP ← (SP − 1) mod 4`; resumes at `stack[SP]`; `ACC ← DDDD`.
- **Overflow**: a 4th nested `JMS` overwrites the deepest return address
  (documented silent circular overflow); a `BBL` from a shallower depth still
  returns correctly.

Source: ai_hw_engineer spec §4.1, §5.5 (stack description from the MCS-4
manual).

## 7. Subtract carry convention

`SUB` and `SBM` both compute `{CY, ACC} ← ACC + ~operand + ~CY`; the add's
carry-out is the new `CY`, so **CY = 1 means no borrow** and an incoming
`CY = 0` injects a pending borrow. This matches the MCS-4 manual's `SBM`
equation `(ACC) + ~(M) + ~(CY)` and is applied to `SUB` identically (the
manual's printed SUB equation omits the inversion; the discrepancy is noted,
not silently resolved — see Open Questions). Example: ACC=5, R=3, CY=0 ⇒
ACC=2, CY=1 (5 − 3, no borrow). A subtract loop clears borrow by setting
`CY = 1` (`STC`) before the first `SUB`.

Source: ai_hw_engineer spec §5.7, cross-validated against MAME MCS-40.

## 8. Undefined opcodes

`1111 1110` and `1111 1111` have no defined function and execute as NOPs
(control proceeds to the next word). This is a documented reconstruction
choice consistent with the real decoder activating no micro-operation; an
assembler should refuse to emit them and a disassembler may render them as
`.byte`.

Source: ai_hw_engineer spec §5.6, §7.

## 9. Page-relative behavior and the end-of-page quirk

The PC increments once per instruction cycle, at phase **A1**. The 12-bit PC
is `{page[11:8], offset[7:0]}`; a 256-byte "page" is one 4001 chip. Several
instructions are **page-local** because they only replace `offset`, taking
`page` from the *current* PC:

- **JCN** and **ISZ** replace only `PC[7:0]` with their second word; the branch
  stays within the page that `PC[11:8]` holds **after both words are fetched**.
- **JIN** replaces only `PC[7:0]` from the pointer pair; same page semantics.
- **FIN** fetches from `{PC[11:8], R0, R1}` — the ROM byte on the *current*
  page indexed by pair 0.

**End-of-page quirk:** because the PC post-increments at A1 of every cycle, a
two-word page-local instruction (JCN/ISZ) whose **first word sits at offset
0xFF** fetches its second word from offset 0x00 of the **next** page, and by
then `PC[11:8]` has already rolled forward — so the branch target lands on the
following page, not the page the instruction began on. Likewise a `JIN`/`FIN`
whose opcode is at offset 0xFF resolves its target page as the next page.
These are not special-cased; they fall out of the A1 increment timing. The
assembler/linker must therefore treat a page-local relocation that straddles
0xFF as an error unless the programmer intends the roll-forward (see
`abi.md` relocations and Open Questions).

`JUN` and `JMS` are full 12-bit and are **not** page-local; they can target
any of the 4096 bytes.

Source: ai_hw_engineer spec §6.1–§6.2 (A1-increment timing and the documented
page-boundary behavior of JCN/JIN/FIN/ISZ).

## 10. Sources

Primary behavioral reference (used for every section above):

- **ai_hw_engineer Intel 4004 design spec**, `design/intel_4004/spec/spec.md`
  §4–§7 — the synthesizable functional reconstruction's behavioral contract,
  itself sourced from the MCS-4 manual and cross-validated against MAME by
  running the original Busicom ROMs (20,192 instruction boundaries, 0
  mismatches). RTL cross-check: `design/intel_4004/src/intel_4004.sv`.
- **Independent emulator cross-check:** `external/chips`
  `src/mcs4/cpu4004.rs` (Andre Weissflog; MIT/zlib — read for cross-checking
  encodings and semantics, **not copied**).

Manufacturer / public sources (as cited by the ai_hw_engineer spec §10):

- **MCS-4 Micro Computer Set Users Manual** (instruction repertoire, JCN
  condition note, DCL/SRC command-line operation, stack description, WPM for
  4008/4009): <http://codeabbey.github.io/heavy-data-1/msc4-manual.pdf>.
  A local scan is held in `ai_hw_engineer/references/MCS4_Data_Sheet_Nov71.pdf`
  (11,184,923 bytes, SHA-256
  `4672da4d81b75c98ef7c8892c9f33d522aa55922d48d6cd4952ecffe9a739622`) and is
  **not** committed here (copyrighted). See `reference/README.md`.
- Instruction repertoire / opcode matrix / cycle counts: e4004
  <http://e4004.szyc.org/iset.html>, pastraiser
  <https://pastraiser.com/cpu/i4004/i4004_opcodes.html>, Pyntel4004
  <https://pyntel4004.readthedocs.io/en/latest/intro/opcodes.html>.
- Semantics cross-validation (subtract carry, JCN bit order, WPM, TCS/DAA):
  MAME MCS-40 core
  <https://github.com/mamedev/mame/blob/master/src/devices/cpu/mcs40/mcs40.cpp>.

No source text is reproduced verbatim beyond device names, signal names, and
instruction mnemonics.
