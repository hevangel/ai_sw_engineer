# IBM PC XT 5160 software catalog

First inventory for [ai_engineer#84](https://github.com/hevangel/ai_engineer/issues/84) (2026-11-06): the firmware, operating system, languages, utilities and applications of the XT profile ([toolchain/spec/profile.md](toolchain/spec/profile.md): 256 KiB, CGA, two diskette drives, BIOS of 1982-11-08, PC DOS 2.00). Not exhaustive. Every original here is copyrighted (IBM, Microsoft or the application's publisher) and is never committed; the "Source" column is where the profile's scripts or a title's `original/provenance.md` fetch it from, with its SHA-256.

## Firmware and boot

| Component | Version | Size | Source (pinned) | SHA-256 | Status |
|---|---|---:|---|---|---|
| XT BIOS | 1982-11-08 (rev 1) | 8 KiB | PCjs `1a91d38e` `machines/pcx86/ibm/5160/rom/bios/1982-11-08/XTBIOS-REV1.rom` | `2798f8a9...` | runs the profile on MartyPC; candidate title (POST, INT 10h/13h/16h services) |
| XT BIOS | 1982-08-16, 1986-01-10, 1986-05-09 | 8/64 KiB | PCjs, same revision, `bios/*` | not pinned | later revisions; the 1986 ROMs include BASIC |
| Cassette BASIC | C1.10 | 32 KiB | PCjs `basic/BASIC110.rom` | `3033d1a5...` | candidate (large: an interpreter) |
| Xebec fixed-disk controller ROM | (PCjs `hdc/`) | 4 KiB | PCjs `machines/pcx86/ibm/5160/rom/hdc/` | not pinned | candidate; needs the fixed-disk model, which neither our emulator nor the profile has |

## Operating system

| Title | Version | Source | Notes |
|---|---|---|---|
| PC DOS | 2.00 (1983-03-08) | pcjs-diskettes `cd343374` `pcx86/sys/dos/ibm/2.00/PCDOS200-DISK1.json`, `-DISK2.json` (180 KiB each) | disk 1 SHA-256 `934e62ce...` (flat image); IBMBIO.COM 4,608 B, IBMDOS.COM 17,152 B, COMMAND.COM 17,664 B. The OS proper is a later, large title |

## Utilities on PC DOS 2.00 disk 1

Sizes from the disk's directory. All are Microsoft/IBM 1983.

| File | Bytes | What | Fit as a title |
|---|---:|---|---|
| **SORT.EXE** | 1,280 | DOS 2 filter: sorts the lines of standard input to standard output; `/R` reverse, `/+n` sort on column n | **done**: `sort/` (#281), selected first (below) |
| MORE.COM | 384 | filter: pages standard input 24 lines at a time | **done**: `more/` (#286) |
| ASSIGN.COM | 896 | redirects drive letters (resident) | resident, patches DOS |
| GRAPHICS.COM | 789 | resident Print Screen for graphics | needs a printer model |
| TREE.COM | 1,513 | lists directories (DOS 2 subdirectories), `/F` with files | **done**: `tree/` (#285) |
| SYS.COM | 1,408 | copies the system files | writes system areas |
| DISKCOMP.COM, DISKCOPY.COM | 2,074, 2,444 | compare/copy whole diskettes | two drives, disk swaps |
| RECOVER.COM | 2,304 | recovers files from bad sectors | needs damaged disks |
| COMP.COM | 2,523 | compares files | **done**: `comp/` (#284) |
| MODE.COM | 3,139 | sets display, printer, serial modes | device-specific |
| BACKUP.COM, RESTORE.COM | 3,687, 4,003 | fixed-disk backup | need the fixed disk |
| EDLIN.COM | 4,608 | line editor | interactive, larger |
| PRINT.COM | 4,608 | background print spooler | resident, printer |
| FIND.EXE | 5,888 | filter: lines containing a string, `/V /C /N` | **done**: `find/` (#283) |
| FORMAT.COM | 6,016 | formats diskettes | writes media; FDC formatting |
| FDISK.COM | 6,177 | fixed-disk partitioning | needs the fixed disk |
| CHKDSK.COM | 6,400 | disk check (the DOS 2 version, with subdirectories) | the 5150's first title was the DOS 1.00 CHKDSK; this is its successor |
| ANSI.SYS | 1,664 | console driver with ANSI escapes | device driver |

## Languages and development tools

| Title | Source | Notes |
|---|---|---|
| BASIC.COM, BASICA.COM | disk 1 (16,256 and 25,984 B) | Disk and Advanced BASIC; large interpreters |
| DEBUG.COM | disk 2 (11,904 B) | debugger |
| LINK.EXE, EXE2BIN.EXE | disk 2 (39,936 and 1,664 B) | linker and converter |
| BASIC samples | disk 2 (`*.BAS`) | programs for BASIC, not machine code |

## Representative applications

Third-party XT-era applications (word processors, spreadsheets, games) are not on any pinned, redistributable source in this repository yet. They are candidates once a legal source with a hash is recorded; none is selected.

## Selection

1. **SORT.EXE** (PC DOS 2.00): child issue in ai_sw_engineer, folder `sort/`. Why first: the smallest complete program that needs what the XT profile adds over the 5150 (DOS 2.00, its handle-based I/O and redirection, the `.EXE` format); a pure function of its input and switches, so the oracle is exact; small enough to specify from the binary in full.
2. Then FIND.EXE (#283), COMP.COM (#284) and TREE.COM (#285), done 2026-11-06: each identical to the original in the four-way matrix (22, 10 and 9 cases). They share `scripts/capture_dos.sh` (one MartyPC boot per case, input files on drive A, optional drive B disks with subdirectories and labels built by pcdisk, prompts answered).
3. MORE.COM (#286), done 2026-11-07: identical on 10 cases; the IBM binary has two instructions the published MORE.ASM lacks (it reprints the character that triggered the prompt).
4. Next candidate: CHKDSK.COM 2.00.
