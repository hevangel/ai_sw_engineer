# MS-DOS 2.0 DEBUG source (reference, not the oracle)

The DEBUG modules from Microsoft's [MS-DOS](https://github.com/microsoft/MS-DOS) repository,
`v2.0/source/` (<https://raw.githubusercontent.com/microsoft/MS-DOS/main/v2.0/source/DEBUG.ASM>
and the same for the others, branch `main`, fetched 2026-11-08), MIT License ([LICENSE](LICENSE),
SHA-256 `b5179f78...`). Kept byte for byte (CR LF line ends, ^Z padding).

| File | Bytes | SHA-256 |
|---|---|---|
| `DEBUG.ASM` | 22,016 | `c498a091b8e11d63...` |
| `DEBCOM1.ASM` | 27,008 | `a3dbb05b1aa8fcb3...` |
| `DEBCOM2.ASM` | 36,087 | `2d73b90047da9767...` |
| `DEBASM.ASM` | 34,304 | `f218367a68974c13...` |
| `DEBUASM.ASM` | 21,888 | `7ab87bb6f4e70f86...` |
| `DEBMES.ASM` | 5,248 | `0cc4d6d5058041fa...` |
| `DEBCONST.ASM` | 38,912 | `73ca6b149f03b61a...` |
| `DEBDATA.ASM` | 2,816 | `2f186bb1e9a7945a...` |
| `DEBEQU.ASM` | 1,280 | `ba5b781bf9e4b52e...` |

The `INCLUDE DOSSYM.ASM` they need is not in this folder. They build DEBUG with the conditional
switches `IBMVER = true`, `SYSVER = false`, `ZIBO = true`, `SETCNTC = true`, `IBMJAPVER = false`.
Read against the binary's disassembly (GNU objdump, `../../spec/spec.md`) the source is the same
program routine for routine (START/DSTRT, COMMAND, INBUF, SCANB/SCANP, GETHEX/HEXIN, ADDRESS,
RANGE/DEFAULT, DISPREG/DISPFLAGS, the disassembler DISASLN and every opcode routine with the
DISTAB and mnemonic tables, the line assembler ASSEM, DUMP, ENTER, FILL, SEARCH, MOVE, COMPARE,
HEXADD, TRACE/GO/DEXIT/REENTER, NAME, DWRITE/DEFIO/EXELOAD, QUIT), with one difference, which the
rewrite follows from the binary:

- **No `"Vers 2.30"` header.** The source has `START: JMP SHORT DSTRT` then `HEADER DB "Vers 2.30"`
  then `DSTRT:`. The IBM binary has the same `JMP SHORT DSTRT` but **no header bytes**: `DSTRT`
  follows immediately, so every offset from DSTRT on is 9 bytes lower than the source would place
  it. The header is never printed (the source's print-header lines are commented out), so the
  behaviour is identical; only the absolute offsets differ.

The rewrite follows the IBM binary and the oracle; the source is used to name the routines and the
tables. `DEBUASM.ASM` and part of `DEBCOM2.ASM` carry the 8087 disassembly, which no case exercises
(the rewrite renders an 8087 opcode as `DB xx`, spec D-DIS).
