# MS-DOS 2.0 CHKDSK source (reference, not the oracle)

`CHKDSK.ASM`, `CHKPROC.ASM` and `CHKMES.ASM` from Microsoft's [MS-DOS](https://github.com/microsoft/MS-DOS)
repository, `v2.0/source/` (<https://raw.githubusercontent.com/microsoft/MS-DOS/main/v2.0/source/CHKDSK.ASM> and
the same for the others, branch `main`), MIT License ([LICENSE](LICENSE), SHA-256 `b5179f78...`). Kept byte for
byte (CR LF line ends, the NUL padding to a multiple of 128 bytes at the end).

| File | Bytes | SHA-256 |
|---|---|---|
| `CHKDSK.ASM` | 26,880 | `2a503e8f1f582e0bcf4323d77edd533d6bc613458db66606814196af37039053` |
| `CHKPROC.ASM` | 40,704 | `a7afdf37d464ecf9bf4d006b5d7f3a7b07f1c72bfb51a6f4ab0c68d478348984` |
| `CHKMES.ASM` | 14,080 | `dae8a032c27cbc2f5e3519104aea2c451bfc5768705ffcf3d24626fc637e38b2` |

They are "CHKDSK Version 2.30" (REV 2.3: split into CHKDSK and CHKPROC; REV 2.2: international support); the
`INCLUDE DOSSYM.ASM` they need is not in this folder. The IBM PC DOS 2.00 `CHKDSK.COM` this title reimplements
(directory date 1983-03-08) is an earlier build. Read against its disassembly it follows these files routine for
routine (DIRPROC, MARKFAT, CHKMAP, FINDCHAIN, CHAINREC, GETENT, AMDONE, CHECKFILES, DSKERR, PROMPTYN, SETSWITCH
and the number routines are the same instructions), with these differences:

- No `JMP SHORT CHSTRT` / `Ver 2.30` at 100h: the binary starts with `PUSH AX`.
- DOS versions 1.54 to **2.00** (`CMP AX,0200h`); the source allows up to 2.11.
- PRINTID has no international support (no function 38h, no INTERNATVARS, US date order only); it prints
  `Volume`, then one message holding the name, `created ` and the month name copied in, and ends with CR LF CR LF,
  where the source prints the month character by character and ends with one CR LF (IDPOST). With a month field of
  0 the copied bytes are ` $a`, which ends the message early (case c04).
- REPORT is in line in the main code instead of a CHKMES routine; the year is printed with CX = 1400h instead of
  1000h (the same output).
- The messages are the same text, with `   Cannot recover .. entry,` ending in its own CR LF.

Used to name the parts in `../../spec/spec.md`; the behaviour itself comes from the IBM binary and the oracle.
