# CHKDSK (PC DOS 2.00): behaviour specification

Derived from the original `CHKDSK.COM` (6,400 bytes, SHA-256 `89f0a7bb...`, [../original/provenance.md](../original/provenance.md)) by reading it in full disassembled with GNU objdump 2.42 (`objdump -D -b binary -m i8086 -M intel --adjust-vma=0x100 CHKDSK.COM`), with Microsoft's MS-DOS 2.0 source of the program ([../reference/ms-dos-2.0/](../reference/ms-dos-2.0/README.md): `CHKDSK.ASM`, `CHKPROC.ASM`, `CHKMES.ASM`, "Version 2.30") to name the parts. REA (spec/software-lifecycle.md) was not used: it needs a Ghidra/IDA/Hopper provider, none is installed here, and the source plus the disassembly cover every routine. The IBM binary is the oracle; where it differs from the source, the binary wins (section "The IBM binary and the source"). Offsets are the `.COM`'s (CS:100h is the first byte); names in capitals are the source's labels. Observations are marked (O), derivations from them (D), inferences not run (I).

## Memory (O)

| Offset | Size | Use |
|---|---|---|
| 1176h | 44 | VOLID, an extended FCB (attribute 08h, name `???????????`); also DIRBUF, the DTA of the directory searches |
| 11A2h | 44 | ALLFILE, the template of each directory's search FCB (extended, attribute 1Eh, drive ALLDRV at 11A9h) |
| 11CEh | 37 | ORPHFCB: drive, `FILE0000CHK`, for function 0Fh when naming recovered chains |
| 11F3h | 1 | SWITCHAR (function 3700h) |
| 11F4h | 2 | ROOTSTR / DIRCHAR: `/` NUL, `\` NUL when the switch character is `/` |
| 11F6h-1212h | | `..` NUL, `.` NUL, DOTENT `.` + 10 spaces, DDOTENT `..` + 9 spaces, HECODE (the last INT 25h/26h AL), FIXMFLG, ERRSUB |
| 1215h | 1 | FRAGMENT: 1 if FCB 1 names a file (the file check at the end), 2 once a fragmented file was reported |
| 1216h | 1 | DIRTYFAT (set by PACK) |
| 1217h-122Ah | words | DIRCNT, DIRSIZ, FILCNT, FILSIZ, HIDCNT, HIDSIZ, BADSIZ, ORPHCNT, ORPHSIZ, LCLUS (sizes in clusters) |
| 122Bh | 1 | DISPFLG: numbers without leading blanks (DISP16BITS, PRINTID) |
| 122Ch | word | CROSSCNT (cross-links seen in the first pass) |
| 122Eh, 122Fh, 1230h, 1231h | 1 each | SECONDPASS, HAVFIX (set, never read), DOFIX (`/F`), NOISY (`/V`) |
| 1232h | 65 | USERDIR: DIRCHAR + the target drive's current directory at entry |
| 1273h | 17 | CONBUF, the function 0Ah buffer (15 characters) |
| 1982h-198Dh | | the old INT 24h and INT 23h vectors, THISDPB (far pointer from function 32h) |
| 198Eh-1995h | | USERDEV (default drive), CSIZE (sectors per cluster), SSIZE, DSIZE (data clusters), MCLUS (DSIZE + 1) |
| 1996h | 14 | NAMBUF (FCB_TO_ASCZ) |
| 19A4h-19B1h | | DOTSNOGOOD, ZEROTRUNC, ISCROSS, OLDCLUS, SRFCBPT, FATMAP (an offset, 19B2h + the FAT's bytes), SECBUF (FATMAP + MCLUS + 2), ERRCNT, STACKLIM (SECBUF + SSIZE + 20) |
| 19B2h | | FAT: the first FAT copy that reads; FATMAP after it (one byte per cluster), then SECBUF (one sector) |

The stack is at the top of the segment (SP stays at FFFEh unless the program's memory is smaller); DIRPROC refuses to recurse below STACKLIM.

## Start (100h-234h)

- (O) 100h: function 30h; AH.AL (swapped) must be 1.54 (0136h) to 2.00 (0200h), else `Incorrect DOS version` CR LF (function 09h), INT 20h.
- (O) 118h: when SP < FFF0h, SP is moved to the top of the program's memory (PSP:0002h) if that is under 64 KiB. 13Ah: function 0Dh (disk reset).
- (O) 140h: AL at entry FFh (COMMAND found an invalid drive in the first parameter): `Invalid drive specification` (no CR LF), INT 20h.
- (O) 155h: switch character (3700h) into SWITCHAR; if `/`, DIRCHAR and USERDIR's first byte become `\`.
- (O) 16Dh: if FCB 1's name (PSP:5Dh) is neither a space nor the switch character, FRAGMENT = 1 (a file name was given: `chkdsk b:*.txt`).
- (O) 182h SETSWITCH (10CEh): over the tail (PSP:80h length, 81h text) `repnz scasb` for the switch character; the letter after it, ORed with 20h: `f` increments DOFIX, `v` NOISY, anything else prints the switch character, the letter and ` Invalid parameter` CR LF and the scan goes on; a switch character as the tail's last byte prints it, a space and the message, and ends the scan. (D: `b:/x` prints `/x Invalid parameter` and checks the disk.)
- (O) 185h: USERDEV = function 19h; the target is FCB 1's drive (PSP:5Ch), or the default. Its number goes into ALLDRV, VOLID, ORPHFCB and the drive letters of `File allocation table bad drive A.` and `ing drive A`.
- (O) 1B2h: function 32h (DL = target, 1 = A); AL = FFh: DONE (the user's drive back), `Invalid drive specification`, INT 20h. Then the target becomes the default drive (0Eh), and, if the DPB's current-directory cluster (DPB+1Ch) is not 0, its text (DPB+1Eh, NUL-ended) is copied after USERDIR's separator.
- (O) 1E8h: INT 23h and INT 24h are saved and hooked (113Ch, 1120h). 212h: CHDIR ROOTSTR; failure: DONE, `Cannot CHDIR to root`, INT 20h.
- (O) 220h: DTA = 19B2h; search first (11h) with VOLID; found: PRINTID (D94h).

## Volume label (PRINTID, D94h; the IBM version)

- (O) DISPFLG = 1; `Volume` (159Ch, function 09h); the label's 11 bytes into the message at 15A4h; the date's month field (bits 8-5) selects three bytes at MONTAB - 3 + 3 x month (MONTAB at 15BEh), copied over the message's 15B8h; then the message from 15A3h: a space, the label, a space, `created `, the three bytes, a space, `$`.
- (D) A month of 0 copies ` $a` (the space and `$` before TCHAR at 15BDh): the message ends right after `created ` and one space, so pcdisk's undated label prints `Volume CHECKED     created  , 1980 12:00a` (case c04). Months 13-15 copy bytes of `Disk error reading FAT` (I: not run).
- (O) The day (bits 4-0) in BCD with CX = 1210h and DISPFLG set: no leading zero, nothing for 0; `, `; the year (bits 15-9 + 1980) in four digits; a space; the hour (bits 15-11): 12 and over sets TCHAR (15BDh) to `p` and 13-23 lose 12, 0 is 12; `:`; the minute in two digits; TCHAR; CR LF CR LF (1284h); DISPFLG = 0.

## The FAT (235h-2F9h)

- (O) From the DPB: SSIZE (+2), CSIZE (+4, plus 1), MCLUS (+0Dh), DSIZE = MCLUS - 1, the FAT's sectors (+0Fh, a byte), its bytes added to FATMAP (so a retry adds them again), SECBUF, STACKLIM.
- (O) For each FAT copy (+8 copies from sector +6, each +0Fh sectors further): INT 25h into 19B2h; the first that reads is used. A failure prints `Disk error reading FAT ` and the copy's digit (`1`, `2`), CR LF. All failing: DSKERR (`read`): Retry starts again at 235h, Ignore gives FATAL `File allocation table bad drive X.`.
- (O) 2D3h: a first byte below F8h asks `Probable non-DOS disk.` CR LF `Continue (Y/N)? `; N goes to the end (AMDONE). 2E6h: FATMAP cleared (MCLUS + 1 bytes); DTA = DIRBUF.
- (O) 2FAh: DIRPROC(0, 0), CHKMAP, CHKCROSS, CR LF, the report (30Ah), AMDONE, INT 20h.

## The walk (DIRPROC, 539h)

DIRPROC(me, parent) checks the current directory (cluster `me`, 0 for the root):

- (O) DOTSNOGOOD = ERRSUB = 0; a 44-byte search FCB on the stack, a copy of ALLFILE; with `/V` in the first pass `Directory ` + PRINTCURRDIR + CR LF.
- (O) Search first (11h). In a subdirectory the first entry must be `.` (attribute 10h, cluster = me, size 0) and the second `..` (attribute 10h, cluster = parent, size 0); a wrong field is reported (with `/V` `B:\DIR\.` CR LF `   Entry has a bad ` + ` attribute`/` link`/` size` CR LF; without, SUBERRP's `B:\DIR` + `   Invalid sub-directory entry.` once per directory), corrected in DIRBUF and written with FIXENT. A missing `.`/`..` is made in slot 0/1 if that slot is free (E5h) and written with `/F`; else DOTSNOGOOD, and a missing `..` ends this directory (`   tree past this point not processed.`, DOTDOTHARDWAY). An empty directory: `   Directory is totally empty, no . or ..` (with `/V`), the same end.
- (O) Each further entry (search next, 12h): a file in the first pass: with `/V` `      ` + PRINTTHISEL + CR LF; MARKFAT with 81h; hidden (02h) or volume label (08h) entries count in HIDCNT/HIDSIZ, others in FILCNT/FILSIZ. A directory in the first pass: MARKFAT with 82h, DIRCNT/DIRSIZ; a directory truncated to nothing by MARKFAT becomes a file (attribute bit 10h cleared, FIXENT). Then CHDIR to its name (FCB_TO_ASCZ) and DIRPROC(its cluster, me); a failed CHDIR prints `Cannot CHDIR to ` + `B:\DIR\NAME` CR LF + `   tree past this point not processed.`. After the child, DOTSNOGOOD asks (first pass) `   Unrecoverable error in directory.` / `   Convert directory to file (Y/N)? `: Y re-reads the entry (GETENT), gives it the chain's size and makes it a file. In the second pass every entry, file or directory, goes through CROSSLOOK instead of MARKFAT.
- (O) At the end of a subdirectory: CHDIR `..`; on failure (with `/V` `   CHDIR .. failed, trying alternate method.`) DOTDOTHARDWAY: the DPB's current cluster = FFFFh, its text loses the last element (or the cluster becomes 0 at the root), CHDIR `.`.
- (D) The walk is depth first in directory order; search next continues in the parent after the child's CHDIRs because DOS 2.00 keeps the directory and the entry number in the search FCB (THISENT, FCB+17h, is the entry number GETENT uses).

## Chains (MARKFAT 7F9h, CROSSLOOK 7C6h)

- (O) A first cluster below 2, above MCLUS, or whose FAT entry is 0: not 0 gives `   First cluster number is invalid,` CR LF `    entry truncated.`, 0 with a size not 0 gives `   Allocation error, size adjusted.`; both set cluster and size to 0 (FIXENT) and ZEROTRUNC.
- (O) Otherwise each cluster of the chain is marked in FATMAP (the head with the given mark, the rest without 80h); a cluster marked already makes a cross-link (CROSSCNT, the old mark gets 10h, the chain stops there, ISCROSS); a link below 2 or above MCLUS (and not FF8h-FFFh) prints `   Has invalid cluster, file truncated.` and the previous cluster gets FFFh (PACK). The count is the number of clusters followed.
- (O) Files that are not cross-linked, in the first pass: the clusters x CSIZE x SSIZE must hold the size and exceed it by less than a cluster; else `   Allocation error, size adjusted.` and the size becomes the allocation (FIXENT). Names are printed by PRINTTHISELERR: CHECKNOFMES first, which prints once, without `/F`, CR LF `Errors found, F parameter not specified.` CR LF `Corrections will not be written to disk.` CR LF CR LF.
- (O) CROSSLOOK (second pass): the chain is followed until a cluster whose mark has 10h; then the name, CR LF, `   Is cross linked on cluster ` and the cluster (DISP16BITS) CR LF. (D) Every file sharing a cluster is named, at the first shared cluster of its chain (case c09).
- (O) All error text goes through EPRINT/CHECKERR, which print nothing in the second pass.

## Lost clusters (CHKMAP 984h, RECOVER 9CCh, FINDCHAIN A34h, CHAINREC AA9h)

- (O) Clusters 2..MCLUS not marked and not free in the FAT: FF7h counts in BADSIZ (mark 04h), others in ORPHSIZ (mark 08h).
- (O) With orphans: CHECKNOFMES, CR LF, `N lost clusters found in ` (DISP16BITS), FINDCHAIN, `M chains.` CR LF, `Convert lost chains to files (Y/N)? ` (PROMPTYN: function 0Ah, CR LF, an empty line asks again, `y`/`n` in either case).
- (O) FINDCHAIN: each unseen orphan heads a chain (mark |= 81h, ORPHCNT); its links are followed while they lead to unseen orphans (mark |= 01h); a link out of range, to itself, to a non-orphan or to an orphan already seen gets FFFh; a link to an earlier head joins the chains (that head loses 80h, ORPHCNT - 1).
- (O) N: the orphans are freed in the FAT (PACK 0); ` bytes disk space freed.` (with `/F`) or ` bytes disk space` CR LF `          would be freed.` (without; LCLUS = the clusters, counted as used in the report), after the byte count (DISPCLUS).
- (O) Y (CHAINREC): for each chain head (mark 89h, in cluster order) the next root entry (from 0, up to the DPB's root entry count) that is free (E5h or 00h): with `/F` the entry gets the first cluster, function 0Dh, then function 0Fh opens ORPHFCB until a name is free (FILE0000.CHK, FILE0001.CHK, ...), the name, 15 zero bytes, the chain's size (GETFILSIZ: clusters x CSIZE, 16 bits, x SSIZE), INT 26h of the sector. Without `/F` nothing is written and the next chain takes the next entry. Too few entries: `Insufficient room in root directory.` CR LF `Erase files in root and repeat CHKDSK.`.
- (O) CHKCROSS (B8Fh): with CROSSCNT, CR LF and the second pass.

## Report (30Ah-3DFh) and end

- (O) Numbers (DISP32BITS, EA2h): 32 bits to BCD; nine columns, leading zeros as blanks (CX = 1810h); with DISPFLG (DISP16BITS) four digits without blanks (CX = 1310h). DISPCLUS: clusters x SSIZE, shifted left while CSIZE's low bits are 0.
- (O) ` bytes total disk space` (DSIZE); if HIDCNT: ` bytes in ` + `N hidden files`; if DIRCNT: ` bytes in ` + `N directories`; if FILCNT: ` bytes in ` + `N user files`; if ORPHSIZ: ` bytes in ` (with `/F`) or ` bytes would be in` CR LF + 10 spaces, then `N recovered files`; if BADSIZ: ` bytes in bad sectors`; ` bytes available on disk` (DSIZE minus all of them and LCLUS) CR LF CR LF; PSP:0002h x 16 ` bytes total memory`; (PSP:0002h - CS) x 16 ` bytes free` CR LF CR LF.
- (O) AMDONE (BA6h): with DIRTYFAT and `/F`, INT 26h of the FAT to each copy (`Disk error writing FAT n` on failure; all failing: DSKERR `writ`); then NOWRITE: function 0Dh, CHDIR USERDIR, with FRAGMENT = 1 CHECKFILES; DONE: the user's drive (0Eh).
- (O) CHECKFILES (C33h): CR LF; DTA = FATMAP; search first/next with FCB 1 (normal FCB: the found entry's cluster at DTA+1Bh): the chain's breaks (a link that is not the next cluster) are counted; a file with breaks: `B:\NAME.EXT` CR LF `   Contains ` (breaks + 1) ` non-contiguous blocks.` CR LF, FRAGMENT = 2. No match: the name of FCB 1, CR LF, `   File not found.`. Else, without any fragmented file: `All specified file(s) are contiguous.`.
- (O) DSKERR (100Eh): `<Write protect|Bad unit|...|Disk>` (HECODE, at most 12), ` error `, `read`/`writ`, `ing drive X` CR LF, `Abort, Retry, Ignore? ` with function 0C01h until `a`, `r` or `i` (either case): Abort ends through RDONE (NOWRITE, INT 20h).
- (O) FATAL (108Bh): `Processing cannot continue.` CR LF, the message, the user's drive, INT 20h. INT_23 (113Ch): the vectors back, FRAGMENT = 0, NOWRITE, INT 20h. INT_24 (1120h): calls the saved handler; on Abort (AL = 2) DONE and INT 20h.

## The IBM binary and the source

The binary follows `CHKDSK.ASM`/`CHKPROC.ASM`/`CHKMES.ASM` routine for routine, with these differences (O):

| Where | Source (Version 2.30) | IBM PC DOS 2.00 binary |
|---|---|---|
| 100h | `JMP SHORT CHSTRT` and `Ver 2.30` | starts at `PUSH AX`; no version text |
| DOS versions | 1.54 to 2.11 (`DOSVER_HIGH EQU 020BH`) | 1.54 to 2.00 (`CMP AX,0200h` at 10Ch) |
| PRINTID | function 38h (international info) and three date orders; the name in VNAME, `created ` in one message, the month printed character by character, ending with IDPOST (CR LF) | no function 38h, US order only; `Volume` printed first (159Ch), then one message with the name, `created ` and the month copied in (15A3h); ends with CR LF CR LF (1284h), so a blank row follows the label line |
| REPORT | a routine in CHKMES.ASM | the same code in line after CHKCROSS (30Ah-3DFh) |
| Year digits | CX = 1000h | CX = 1400h (same output: no suppression) |
| INTERNATVARS | a 38h buffer in DATA | absent |

The messages are the same text; the IBM binary keeps `   Cannot recover .. entry,` with its own CR LF instead of sharing IDPOST.

## The rewrite

`src/lib.rs` follows these routines with the same state, the same order of output and of disk writes, and the same arithmetic (BCD conversion and leading-zero state of DIGIT, 16-bit sizes in GETFILSIZ, the 16-bit entry offset in GETENT). Deliberate differences:

| ID | Difference | Why | Effect |
|---|---|---|---|
| D1 | The FAT, FATMAP and SECBUF share one 12,288-byte static buffer, laid out as in the original (FATMAP after the FAT, SECBUF after MCLUS + 2 map bytes); a disk whose layout does not fit gives FATAL `Insufficient memory.` before the FAT is read; DIRPROC has no STACKLIM check (the recursion is Rust's, on the stack at the top of the segment) | Rust objects are limited to 32,767 bytes with 16-bit pointers; the original uses the free memory up to its stack | none for any FAT12 disk with 512-byte sectors (the largest, 12 FAT sectors and 4,085 clusters, needs 10,767 bytes); a deeper tree than DOS's 63-character current directory allows cannot occur |
| D2 | The DTA is set (1Ah) before each search; the volume search, the walk and CHECKFILES have their own records; CHECKFILES searches with a copy of FCB 1 | the DTA lives in the core's state | none visible (the original's DTAs overwrite the FAT and FATMAP after their use) |
| D3 | INT 25h/26h are made by `src/disk.s` (`chkdsk_disk_io`), which removes the flags they leave on the stack | the shared runtime has INT 21h only, and Rust has no inline assembly here | none |
| D4 | The INT 23h handler is a Rust function: disk reset, CHDIR to the saved USERDIR, the saved drive, INT 20h (INT_23 without restoring the vectors, which DOS restores from the PSP); INT 24h is not hooked | no code at fixed addresses | the same for Ctrl-Break; on a critical error DOS's own handler runs, so Abort ends CHKDSK without selecting the user's drive again (the original's INT_24 calls DONE); not exercised by a case |
| D5 | `build/chkdsk.com` is 12,534 bytes, the original 6,400 | the llvm-i8086 code for the core is 10,600 bytes against the original's 4,214 of hand-written assembly | a disk holding the rewrite in place of CHKDSK.COM has less free space: on PC DOS 2.00 disk 1 (512-byte clusters) 25 clusters instead of 13, so `chkdsk a:` there reports 6,144 bytes more in user files and 6,144 bytes less available. No case checks drive A (c15 runs `a:chkdsk` with drive B the default) |
| D6 | The target's INT 21h calls go through two out-of-line wrappers of `ibm_pc_dos::int21`; messages are printed with function 09h from `$`-terminated constants, as in the original | code size | none |
| D7 | HAVFIX is not kept | the binary sets it and never reads it | none |

Assumption ledger: `ASSUMPTION (spec D1)` in `src/lib.rs` is D1 above.
