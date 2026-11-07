# MS-DOS 2.0 FORMAT source (reference, not the oracle)

`FORMAT.ASM`, `FORMES.ASM` and `GENFOR.ASM` from Microsoft's [MS-DOS](https://github.com/microsoft/MS-DOS)
repository, `v2.0/source/` (<https://raw.githubusercontent.com/microsoft/MS-DOS/main/v2.0/source/FORMAT.ASM> and
the same for the others, branch `main`), MIT License ([LICENSE](LICENSE), SHA-256 `b5179f78...`). Kept byte for
byte (CR LF line ends, the NUL padding at the end).

| File | Bytes | SHA-256 |
|---|---|---|
| `FORMAT.ASM` | 45,094 | `fedea22ee474777d1062fcf437bcfb0f22085b8aa3a65d44fee2ef860208d32d` |
| `FORMES.ASM` | 4,529 | `3fe7a6010ebd4a2801eeb63e50fedf4cbcb5915591c5620c490606b13c418afd` |
| `GENFOR.ASM` | 4,096 | `7efa0535cac1279d2e5c6d869cb1c78ab4aa216b1cbd839d2fa709c49f47f6a6` |

`FORMAT.ASM` (the main module, "Vers 2.10", REV 2.10 of 5/1/83: "Re-do to transfer system on small memory systems")
and `FORMES.ASM` (its messages, WAITYN and REPORT) are Microsoft's generic part. FORMAT is built from them and an
OEM module that supplies INIT, DISKFORMAT, BADSECTOR, WRTFAT and DONE and the data FATID, STARTSECTOR, SWITCHLIST,
FREESPACE, FATSPACE and HARDFLAG. `GENFOR.ASM` is Microsoft's generic OEM module (no physical formatting: it only
reads every sector). **IBM's OEM module for PC DOS 2.00 is not published**; in the IBM binary it is 0E94h-1241h
(code) and 1242h-187Fh (its messages, the BPB table, two boot records), and it is ported from the disassembly
([../../spec/spec.md](../../spec/spec.md)).

What the IBM PC DOS 2.00 `FORMAT.COM` (6,016 bytes, directory date 1983-03-08) shares with these files, read
against its disassembly:

- **Matches** (the same instructions, IBMVER branches taken): START's stack (`MOV SP,0BF1h`), the ASSIGN check,
  the drive check on AL, DRVSPEC, the switch parse (NXTSWT, GETPARM, SCANOFF, INVALID) with the IBM `/B` rule
  (SAVSWT: /B turns /S off and /8 on), SYSLOOP, GETTRK/TRKFND/CLRTEST/SYSTEST, PACK, DRTFAT/CLEARED/FATWRT,
  STATUS, DISP32BITS, OUTWORD/OUTBYTE/DIGIT, CONVWRD, UNSCALE, PRINT, MORE, CRLF, FEXIT, SYSPRM (IBMVER: drive A
  when the default is not a diskette drive), DSKPRM, FDPB, CLEAR (without the OLDSW create loop), VOLID; FORMES's
  WAITYN, REPORT and messages (with ASGERR).
- **Differs** (the IBM binary is an earlier build):
  - no `JMP SHORT FSTRT` / `Vers 2.10` at 100h; DOS 2.00 or later is accepted (`CMP AX,0200h`; the source asks
    for 2.11, `DOSVER_HIGH EQU 020BH`);
  - the system files are read with FCBs (open 0Fh, random block read 27h, record size 1) into a fixed buffer at
    CS:3050h, IBMDOS.COM first, then IBMBIO.COM, and COMMAND.COM separately (75Bh); a COMMAND.COM that does not
    fit is read after formatting (83Ch). The source's REV 2.10 redo (SETBLOCK/ALLOC, handles, FILESTRUC, partial
    reads) is absent;
  - WAITKY makes one call (0C08h; the source adds 0C00h);
  - BAD100's cluster arithmetic: the clusters are marked from `ceil((first + count - STARTSECTOR) / CLUSSIZ)`
    downwards (one lower than the source's `+ 2` numbering);
  - DRTFAT calls GETSIZE whenever it calls CLEAR (the source only with /S); FNDSIZ adds one sector instead of
    rounding up to the cluster (the same for 2-sector clusters);
  - REPORT prints `used by system` when the low word of SYSSIZ is not 0 (the source tests both words);
  - the OLDSW switch is `/8`, and its E5h directory is made by DONE with INT 25h/26h, not by creating files.

Used to name the parts in [../../spec/spec.md](../../spec/spec.md); the behaviour itself comes from the IBM binary
and the oracle.
