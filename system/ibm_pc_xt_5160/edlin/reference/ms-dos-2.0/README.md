# MS-DOS 2.0 EDLIN source (reference, not the oracle)

`EDLIN.ASM`, `EDLPROC.ASM` and `EDLMES.ASM` from Microsoft's [MS-DOS](https://github.com/microsoft/MS-DOS)
repository, `v2.0/source/` (<https://raw.githubusercontent.com/microsoft/MS-DOS/main/v2.0/source/EDLIN.ASM> and the
same for the others, branch `main`, fetched 2026-11-08), MIT License ([LICENSE](LICENSE), SHA-256 `b5179f78...`).
Kept byte for byte (CR LF line ends; `EDLIN.ASM` and `EDLPROC.ASM` end with ^Z padding).

| File | Bytes | SHA-256 |
|---|---|---|
| `EDLIN.ASM` | 55,115 | `5751619c71228b8ceaba3866056118f1ab12d1b05ab90b8c89f8abac5f8a7c62` |
| `EDLPROC.ASM` | 13,190 | `dc425bef63c4c9c7639cd9ba5ec4b63f1440d96900c4670239b50662124e2f43` |
| `EDLMES.ASM` | 3,200 | `2c244df61181f7035911f2c51789e0f8d60ac73c706f4e5035f3f28ad8a925b0` |

They are **revision 14** (7/23/83, N. Panners: EDLIN split into EDLIN and EDLPROC); the `INCLUDE DOSSYM.ASM` they
need is not in this folder. The IBM PC DOS 2.00 `EDLIN.COM` this title reimplements (directory date 1983-03-08)
is **revision 12**: its constants read `BAK***MAUlloa/Microsoft/V20***12` (ED1h), the source's `...***14`.
Read against the binary's disassembly (GNU objdump, `../../spec/spec.md`) the source is the same program routine
for routine (the start and its path handling, PARSE, GETNUM, the command table and its order, FINDLIN, SCANLN,
SHOWNUM/CONV10, DISPLAY, LOADBUF, LIST, PAGE, APPEND/SCANEOF/CHECK_END, EWRITE/WRTADD, DELETE, NOCOM, REPLACE,
FNDFIRST/FNDNEXT/GETTEXT, the S and R loops, UNQUOTE, END_INS, ENDED, DELBAK, REST_DIR, INT_24, the messages),
with these differences, which the rewrite follows from the binary:

- **No patch area, no header.** The source starts with `JMP SIMPED`, a 0E00h-byte `edl_pad` and `Vers 2.14`; the
  binary jumps to 108h over a 3-byte NONAME and has IBM's copyright text after the code (E5Ah-ED0h). Every
  offset differs: the data group starts at ED1h, the text at **1539h**.
- **No read-only protection.** `roprot equ true` in EDLIN.ASM adds a function 43h attribute check, ROTABLE and
  QUIT1; the binary has none of them (the start goes from the name copy straight to the directory scan, 177h;
  the table's last entry is QUIT, E13h). The `Invalid operation: R/O file` text is in the binary, unused.
- **The version test** is `JGE` (signed) where the source has `JAE` (115h).
- **QUIT and QUERY** read their key with function 01h (E1Ah, E45h), not 0C01h: no type-ahead flush.
- **INSERT's memory check** (C94h-CA5h) is `DX = SI + CX + 1; CMP DX,BP; JB` with SI in EDITBUF, so it compares the
  end of the typed line in the line buffer, not in the text, with the free space, and never fails while the text
  is above the buffer; on failure it jumps to MEMERR without END_INS. The source adds the length to DI, checks
  carry and zero, and calls END_INS first (`MEMERRJ1`).
- **MERGE** (BBAh, T), before revision 13's "fixed bug in Merge which lost char if not ^Z": after a read that
  reached the end of the file (AL = 1) it always drops the last byte read (`DEC CX`, C32h), ^Z or not; and RESTORE
  (C37h) moves the text above the gap down and sets ENDTXT **without storing a ^Z there** (the source's
  `MOV BYTE PTR [DI],1AH`). The byte at ENDTXT is then whatever memory held; E writes it as the file's last byte
  (case e09 shows it: `[`, a byte of COMMAND.COM left in memory by the boot).
- **MOVE and COPY** are an older BLKMOVE (AADh-B95h), without the source's PTR_1-PTR_3/COPYSIZ variables, MUL of
  the count or full-buffer checks (revision 13 "fixed bug in Copy to correctly check for full buffers"): the block
  (PARAM1-PARAM2) is copied to the top of memory below LAST first (12B6h keeps the byte below the copy, 12B4h its
  size); a move then lowers PARAM3 by the block's lines if its line is at or past PARAM2 and deletes the block with
  DELETE; then, once for M and PARAM4 times (at least once) for C, MOVEFILE opens a gap at line PARAM3, the copy is
  put in it (MEMERR if it does not fit below the moved text) and the rest of the text is moved down after it.
  CURRENT is PARAM3 (the first line of the last copy).
- The data group has no `fake_fcb`: FCB2 is at 1441h, FCB3 at 1466h, QFLG at 14B0h, the stack 14B9h-1538h; and
  PARAM1-COMBUF+3 (1294h-12BFh) lie inside the file, so their initial values are the file's bytes there (ONE4TH,
  12ACh, is 6176h; it is read by `W` without a number on a new file).

Microsoft's own build of revision 12 in the same repository, `v2.0/bin/EDLIN.COM` (4,389 bytes, SHA-256
`799767e5...`), has the same instructions as IBM's (about 1,400, compared mnemonic by mnemonic) with a
`Vers 2.00` header instead of IBM's copyright text and the addresses moved accordingly. The rewrite follows the
IBM binary and the oracle; the source is used to name the routines.
