# IBM PC BASIC: I/O contract

What the original pair (ROM Cassette BASIC C1.10 + BASIC.COM D2.00) does to the machine, and what the rewrite
must do the same. Behaviour: [spec.md](spec.md). Phase 1, 2026-11-09.

## Memory on the oracle machine (256 KiB, PC DOS 2.00)

| Range | Original | Rewrite |
|---|---|---|
| PSP (DOS gives the program all memory) | BASIC.COM's PSP; the command tail is copied to DS:01F9h | the same PSP; the tail is parsed from PSP:0080h |
| PSP+10h .. PSP+403h paragraphs | BASIC.COM's code (16,256 bytes from PSP:0100h) | the rewrite's code and Rust data: a multi-segment `.EXE` (spec D1). **Open decision M-1**: where it is loaded (see below) |
| PSP+403h, 64 KiB | BASIC's data segment (DS, DEF SEG default): spec §3 | a 64 KiB far segment with the same offsets for the observable data (spec D2) |
| F600:0000-7FFF | ROM BASIC, called through BASIC.COM's far-jump table and RAM hooks | not used |

The original's DS is `PSP+403h` (the PSP segment depends on what DOS has resident), so DEF SEG's segment and `VARPTR` are offsets in it. Programs cannot see the absolute segment
without PEEKing low memory; none of the cases does.

**Bytes free** must come out as 61766 with 256 KiB: the data segment must be a full 64 KiB (it is when at
least 64 KiB remain after the code, as on the oracle machine). The rewrite asks DOS for its blocks with
functions 4Ah/48h (the original: 4Ah with BX=FFFFh to learn the size, then 4Ah to take it all).

## Keyboard and screen

| Original | Use | Rewrite |
|---|---|---|
| INT 16h (ROM BASIC's keyboard routines, through BIOS) | read keys, check for Ctrl-Break / Ctrl-C (INT 1Bh hooked) | the same BIOS calls |
| INT 10h function 0Fh at start | the video mode (saved at DS:0048h, 071Ah); 80 or 40 columns | the same |
| Video memory B800h (CGA) / B000h (MDA), written directly and through INT 10h | text, cursor (INT 10h 02h), scrolling, the key line on row 25 | the same addresses and attributes (7 = normal), so the transcripts and a framebuffer hash agree |
| INT 1Bh, 1Ch, 0, 4 vectors | saved at start (DS:0712h-071Ah, from 0000:0000-0012h and 006Ch-0072h), restored at SYSTEM | the same set |

The screen editor keeps a per-row "logical line continues" table (GW `SCNDRV` terminator table) so Enter
returns the whole logical line.

## DOS calls BASIC.COM makes (B, from the disassembly; first address of each)

| INT 21h | Use |
|---|---|
| 05h | LPT output (LPRINT, LLIST) — the ROM uses INT 17h for the printer too |
| 09h | the two start-up refusals |
| 0Dh | disk reset (RESET, before SYSTEM / SHELL) |
| 0Eh, 19h | select / current drive |
| 1Ah | set DTA (FILES' search) |
| 2Ah-2Dh | DATE$, TIME$ (get and set) |
| 30h | DOS version (≥ 2 required) |
| 33h | Ctrl-Break checking state (saved and turned off at start, restored at exit) |
| 36h | disk free space (FILES' `Bytes free`) |
| 37h | the switch character (`/` or `-`, so paths use `\`) |
| 3Ah (and 39h, 3Bh by AX) | RMDIR, MKDIR, CHDIR |
| 3Ch, 3Dh, 3Eh, 3Fh, 40h, 42h | create, open, close, read, write, seek — **handle-based I/O for every file** (no FCB file calls) |
| 41h | KILL |
| 44h | IOCTL: is a handle a device (SCRN:, CON-like files) |
| 47h | current directory (FILES' header `B:\`) |
| 4Ah, 4Bh | memory and EXEC (SHELL with COMSPEC from the environment) |
| 4Ch | SYSTEM |
| 4Eh, 4Fh | FILES (find first / next) |
| 56h | NAME |
| 57h | file date and time |

Every disk call's DOS error is mapped to a BASIC error (2 → 53 File not found, 3 → 76 Path not found, 4 → 67
Too many files, 5 → 75 Path/File access error, 17 → 74 Rename across disks, ...; the table is BASIC.COM's,
to be read in milestone M5, U6). Critical errors (INT 24h) become 70-72 and 57.

### File layout and records

- Sequential files are buffered in their file data block in DS (128-byte records by default, /S); LOC counts
  128-byte records (spec §11). A CLOSE of an output file writes a 1Ah after the text (G).
- Random files: record length from OPEN (default 128, at most /S); PUT/GET seek with function 42h to
  `(record-1)*len`; LOF is the file size in bytes.
- BLOAD/BSAVE: 7-byte header `FD seg off len` (G), not covered.

## Program files

| Form | Bytes |
|---|---|
| tokenized | `FF` + TXTTAB..VARTAB + `1A` |
| protected | `FE` + encoded TXTTAB..VARTAB + `1A` (spec §11) |
| ASCII | LIST lines, CR LF each, then `1A` |

## Devices

`SCRN:`, `KYBD:`, `LPT1:`-`LPT3:`, `COM1:`, `COM2:`, `CAS1:` (cassette, refused under DOS: `Device Unavailable`?
U7), and disk files with drive and path.

## Unknowns

| ID | Unknown |
|---|---|
| U6 | BASIC.COM's DOS-error-to-BASIC-error table |
| U7 | What CAS1: gives under BASIC.COM |
| M-1 | Load address of the rewrite (decision for the user, plan §"Decisions"): low (code right after the PSP, data segment above it: Bytes free stays 61766 at 256 KiB, absolute segments differ) or loaded so that the data segment sits exactly at PSP+403h (code above it); the cases cannot tell the two apart |
