# FORMAT (PC DOS 2.00): behaviour specification

Derived from the original `FORMAT.COM` (6,016 bytes, SHA-256 `cd86654e...`, [../original/provenance.md](../original/provenance.md)) by reading it in full disassembled with GNU objdump 2.42 (`objdump -D -b binary -m i8086 -M intel --adjust-vma=0x100 FORMAT.COM`), with Microsoft's generic MS-DOS 2.0 source ([../reference/ms-dos-2.0/](../reference/ms-dos-2.0/README.md): `FORMAT.ASM`, `FORMES.ASM`, "Vers 2.10"; `GENFOR.ASM`, the generic OEM module) to name the parts of the main module. IBM's OEM module (0E94h-1241h) has no published source and is described from the disassembly alone. REA (spec/software-lifecycle.md) was not used: it needs a Ghidra/IDA/Hopper provider, none is installed here, and the disassembly is small enough to read in full. The IBM binary is the oracle; where it differs from the source, the binary wins ([reference README](../reference/ms-dos-2.0/README.md) lists the differences). Offsets are the `.COM`'s (CS:100h is the first byte); names in capitals are the source's labels. Observations are marked (O), derivations from them (D), inferences not run (I).

## Memory (O)

| Offset | Size | Use |
|---|---|---|
| 100h-0C1Bh | | the main module (START to WAITYN); the stack is internal: SP = 0BF1h |
| 39Eh-3D4h | 55 | a boot-message stub (prints `Non-System disk or disk error` with INT 10h); `/B` writes IBMBIO.COM from here |
| 93Fh | 4 | ROOTSTR `x:` + SWTCH (941h, the switch character from 3700h) + NUL; SWTCH NUL alone is the root of the default drive |
| 943h, 944h, 945h, 946h | 1 each | DBLFLG, CLEARFLG, DRIVE (target, 0 = A), DEFALT |
| 947h-962h | | the date/time being written (947h, 94Fh), the sources' dates (949h BIO, 94Bh DOS, 94Dh COMMAND) and times (951h, 953h, 955h), COMMAND's DTA (957h), the sizes (959h BIO, 95Bh DOS, 95Dh COMMAND), the memory left (95Fh), COMMAND read (961h = FFh), the drive COMMAND is read from (962h) |
| 963h | 65 | USERDIR: `/` + the default drive's current directory (SAVUDIRS) |
| 9A6h, 9D4h | 44 + 2 each | extended FCBs (attribute 07h) `IBMBIO  COM`, `IBMDOS  COM`, each followed by the word of the file's size |
| 0A02h | 38 | the FCB `COMMAND COM` |
| 0A28h | 44 | VOLFCB: extended FCB, attribute 08h, 11 spaces |
| 0A56h | 44 | ALLFILE: extended FCB, attribute FFh, `???????????` |
| 0A83h-0A9Eh | words | SWITCHMAP, SWITCHCOPY, CLUSSIZ (A8Bh), SECSIZ (A8Dh), SYSSIZ (A8Fh, 32 bits), FDSKSIZ (A93h, 32 bits: clusters, then the total in bytes), BADSIZ (A97h), SYSTRKS (A9Bh), SECTORS (A9Dh) |
| 0A9Fh | 82 | INBUFF: the function 0Ah buffer (80 characters) |
| 0C1Ch-0E8Bh | | the messages (FORMES) |
| 0E90h | 4 | two words, 1650h and 1450h, that no code reads |
| 0E94h-1241h | | IBM's OEM module: WRTFAT (0E94h), INIT (0EBEh), DONE (0F27h), DISKFORMAT (0FAAh), BADSECTOR (1183h) |
| 1242h-1418h | | the OEM's code tail and messages (`Formatting...`, `Format complete`, the INT 13h errors) |
| 12F4h | 6 | SWITCHLIST: 5, `BV81S` (a letter's bit is 1 shl its distance from the end: S 01h, 1 02h, 8 04h, V 08h, B 10h) |
| 12FAh, 130Ch, 131Eh, 1330h | 18 each | the BPBs from sectors per cluster on: 160 KiB (/1/8), 320 KiB (/8), 180 KiB (/1), 360 KiB |
| 1419h | 36 | the track's sector IDs for INT 13h AH=05h (C, H, R = 1-9, N = 2) |
| 143Dh-144Fh | | HARDFLAG, the physical drive, the track (word), the head, track 0 bad, the tries, STARTSECTOR (1444h), the tracks (1446h), a far pointer 0070:xxxx (1448h), the system-file buffer offset 3050h (144Ch), FATSPACE 1850h (144Eh) |
| 1450h | 512 | a DOS 1.10-style boot record (`JMP 27h`, `Microsoft,Inc`), written by DONE for /8 without /S |
| 1650h | 512 | the PC DOS 2.00 boot record (`JMP 2Ch`, `IBM  2.0`); DISKFORMAT copies the BPB into it at 165Dh |
| 1850h- | | FATSPACE: the FAT buffer, past the end of the file (1880h): its bytes beyond what CLEAR clears are whatever memory held |
| 3050h- | | the system-file buffer: IBMDOS.COM, IBMBIO.COM, COMMAND.COM, up to the top of the program's memory (at most 64 KiB) |

## Start (100h-234h)

- (O) 103h: function 30h, swapped, must be 2.00 or later, else `Incorrect DOS version` CR LF, INT 20h.
- (O) 118h: if INT 21h's vector is xxxx:0122h and xxxx:0109h holds 0807h (ASSIGN.COM), its table at 103h-108h must be 01h-06h, else `Cannot format an ASSIGNed drive. `, INT 20h.
- (O) 15Ch: AL at entry FFh: `Invalid drive specification` (no CR LF), INT 20h.
- (O) 169h: DEFALT = function 19h (also 962h); the target is FCB 1's drive or the default; INT 23h is hooked (93Ah: FEXIT).
- (O) 18Fh: the switch character (3700h) into SWTCH; the tail (PSP:81h) is scanned: blanks and tabs skipped; the switch character and a letter (upper-cased unless below `A`, a signed compare) found in SWITCHLIST sets its bit; CR ends; anything else must be one character and `:`, once (the drive); otherwise `Invalid parameter` and FEXIT. (D: `b:/x` prints `Invalid parameter`, case f12.)
- (O) 1F5h: `/B` turns /S off and /8 on. SWITCHMAP is stored.
- (O) 205h, /S: SAVUDIRS (907h: `/` and, when DPB+1Ch of the default drive is not 0, its text from DPB+1Eh); CHDIR to SWTCH (the default drive's root); READDOS (629h) and COMMAND (75Bh); a file that does not open: SYSPRM (546h, `Insert DOS disk in drive x:` CR LF `and strike any key when ready`, key 0C08h, CR LF; drive A when the default is beyond the diskette drives of INT 11h) and again.
- (O) READDOS: the memory left is FFFFh, or (PSP:0002h - CS) x 16 if under 1000h paragraphs; minus 3050h. IBMDOS.COM then IBMBIO.COM are opened (extended FCB, attribute 07h) and their dates and times kept; DTA 3050h; random record 0, record size 1; random block read (27h) of the memory left: AL must be 1 (end of file), else `Insufficient memory for system transfer` and FEXIT; CX is the size. IBMBIO.COM follows at 3050h + its size.
- (O) 75Bh: COMMAND.COM (normal FCB) at 3050h + both sizes; its size from the FCB; read with what is left; AL = 1: read (961h = FFh), else it is read after formatting.
- (O) 226h: INIT; carry: `Format failure` CR LF CR LF, FEXIT.

## INIT (0EBEh)

- (O) The physical drive is DRIVE; INT 11h bits 7-6 give the diskette drives - 1; 0 means one drive (physical drive 0). DRIVE beyond them: HARDFLAG = that count, INT 13h AH=08h DL=80h adds DL; still beyond: `Disk not compatible`; /1 or /8: `Parameter not compatible with fixed disk`; both with carry.
- (O) A diskette: DRIVE into 0050:0004 (DOS's single-drive A/B byte); /8 with /V and without /S: `Parameters not compatible`, carry.

## SYSLOOP (23Ch) and the disk

- (O) BADSIZ, SYSSIZ, DBLFLG, CLEARFLG = 0, SWITCHMAP = SWITCHCOPY, the letter into the messages; DSKPRM (585h): `Insert new diskette for drive x:` CR LF `and strike any key when ready` (a fixed disk: `Press any key to begin formatting x: `), key 0C08h (one call), CR LF CR LF.
- (O) DISKFORMAT (0FAAh), diskette: track 0, 40 tracks; the BPB for the switches (/1/8, /8, /1, none) copied to the boot record at 165Dh; STARTSECTOR = FATs x FAT sectors + reserved + the root's sectors (rounded up); the head is the last; `Formatting...`; INT 13h reset (retried while 1443h, left by the last operation, is not 0); track 0 formatted on each head from the last (INT 13h AH=05h, AL = sectors per track, the nine IDs moved to 0060:0000 for the call, ES:BX = 0060:0000, and back; three tries each); a head that fails: track 0 bad. Then track 0 verified on each head (AH=04h, sector 1, AL = sectors per track, three tries): a failure on head 0 is track 0 bad; on head 1 the layout becomes single-sided (media - 1, STARTSECTOR - 3 in its low byte, 64 root entries, one head, half the sectors) and verifying goes on. Track 1 next.
- (O) INT 13h errors (1159h): 03h `Attempted write-protect violation`, 80h `Drive not ready`: reset, return with carry from DISKFORMAT/BADSECTOR (`Format failure`, SYSLOOP); others: reset, one try less.
- (O) (D) The XT BIOS formats with the disk base table's sectors per track (9), so the nine IDs are all written and an /8 disk has a ninth sector, filled with F6h, on every track (the oracle images show it); only verify uses AL.
- (O) BADSECTOR (1183h): track 0 bad: AX = sectors per track, BX = 0. Otherwise each remaining track and head is formatted and verified (three tries each); a failure returns AX = sectors per track and BX = (track x heads + head) x sectors per track - hidden, the next head prepared. After the last track: the boot record (1650h) to sector 0 (INT 26h; failure `Unable to write BOOT`, carry), `Format complete` CR LF, AX = 0.
- (O) GETTRK (281h): a bad range below STARTSECTOR: `Track 0 bad - disk unusable`, FRMTPROB. Otherwise CLEAR once; with /S, GETSIZE once and SYSTRKS = SYSSIZ / SECSIZ + STARTSECTOR; a bad range at or below it: `Disk unsuitable for system disk`, /S off, SYSSIZ low word 0. The bad clusters: `ceil(SECTORS / CLUSSIZ)` of them, from cluster `ceil((BX + SECTORS - STARTSECTOR) / CLUSSIZ)` downwards, each PACKed FF7h, BADSIZ + bytes per cluster unless it was FF7h already. (D: one lower than the source's numbering; not exercised.)
- (O) DRTFAT (367h): without CLEAR so far, CLEAR and GETSIZE; WRTFAT (failure: `Track 0 bad - disk unusable`, FRMTPROB).
- (O) CLEAR (5C3h): FAT bytes 0-2 = media | F8h, FFh, FFh; disk reset; WRTFAT; FDPB (59Fh: function 32h for DRIVE + 1: clusters = +0Dh - 1, cluster size = +4 + 1, sector size = +2; AL = FFh: `Invalid drive specification`, FEXIT); the clusters x 1.5 (rounded up) bytes from FAT+3 zeroed; disk reset; WRTFAT; CHDIR ROOTSTR; delete (13h) ALLFILE on the target.
- (O) WRTFAT (0E94h): INT 26h of the FAT buffer, FAT sectors of the boot record's BPB, to sector 1 and to sector 1 + FAT sectors.
- (O) (D) The FAT buffer is at 1850h, past the file's end; CLEAR sets only the FAT entries, so the rest of the last FAT sector is what memory held there. On the oracle it is PC DOS 2.00's COMMAND.COM: CS:X holds COMMAND.COM's byte X + 0BC0h (the image COMMAND's own load left there at start-up, 0CCh paragraphs below FORMAT's segment). The oracle's FAT sectors show COMMAND.COM bytes 2626h- (360 KiB), 2622h- (180 KiB), 25ECh- (320 KiB) and 25E9h- (160 KiB) after the cleared bytes, in both copies (spec D2).
- (O) (D) DOS 2.00's delete of `???????????` with attribute FFh matches every entry of the formatted root (F6h bytes, whose attribute has the volume bit) and marks it never used (00h, DOS's DELALL), so every root entry starts with 00h and keeps F6h after it.
- (O) FATWRT (382h): /B: IBMBIO.COM of 780h bytes from 39Eh and IBMDOS.COM of 1900h bytes from 3050h are written (6F4h) and no message; /S: 6ECh (IBMBIO.COM from 3050h + DOS's size, then IBMDOS.COM from 3050h: each created with its FCB on the target, random record 0, record size 1, block write of its size, the source's date and time put in the FCB, closed) and 83Ch (COMMAND.COM: read now if it did not fit, after SYSPRM if needed and DSKPRM again when it comes from the target drive; created, written, dated, closed; then READDOS again for the next disk), `System transferred` CR LF (always after 83Ch, its carry ignored).
- (O) (D) DOS writes a file's last partial sector with the bytes that follow the data in FORMAT's buffer: on the oracle IBMDOS.COM's last sector ends with the first 256 bytes of IBMBIO.COM (which follows it at 3050h + 17,152) and COMMAND.COM's with zeros.
- (O) STATUS (3E9h): CR LF; VOLID; disk reset; DONE (carry: FRMTPROB); REPORT; MORE.
- (O) VOLID (7C1h), with /V: VOLFCB's drive; `Volume label (11 characters, ENTER for none)? `; function 0Ah; CR LF CR LF; an empty line ends; else 11 spaces after the line, its first 11 bytes to VOLFCB's name, create (16h): AL not 0: `Invalid characters in volume label` CR LF and the prompt again (case f09); else close and CR LF. DOS stamps the label with its clock: the oracle's labels say 0:00:24 (f04, f09) and 0:00:28 (f08) of 1-01-1980.
- (O) DONE (0F27h), only with /8: 7 sectors from sector 3 (the root) into the FAT buffer; the attribute bytes of entries 0 and 1 set to 06h; from the last root entry backwards, each entry whose first byte is 00h gets E5h until one is not 00h; the 7 sectors written back. Without /S, the boot record at 1450h (with 0103h at offset 3 unless the media byte is FEh) is written to sector 0 (spec D1).
- (O) REPORT (3FBh): FDSKSIZ x SECSIZ shifted left for CLUSSIZ (UNSCALE) is the total; `nnnnnnnnn bytes total disk space`; SYSSIZ (if its low word is not 0) ` bytes used by system`; BADSIZ (if not 0) ` bytes in bad sectors`; total - BADSIZ - SYSSIZ ` bytes available on disk` CR LF CR LF. Numbers (DISP32BITS, 473h): 32 bits to eight BCD digits in BX:BP and a ninth column in AL (the carries out of BX), CX = 1810h: leading zeros as blanks, the last digit always printed.
- (O) MORE (522h): HARDFLAG: FEXIT. WAITYN (0C00h): `Format another (Y/N)?`, 0C01h (flush, read with echo), ANDed with DFh: Y: CR LF CR LF and SYSLOOP; N: FEXIT; anything else CR LF and the question again.
- (O) FEXIT (541h): with /S in SWITCHMAP, CHDIR USERDIR; INT 20h. INT_23 (93Ah) is FEXIT.

## Sizes (D, from the code and the oracle)

| Switches | Layout | Clusters | FAT bytes cleared | Total | System (/S) |
|---|---|---|---|---|---|
| none | 360 KiB, media FDh, 2 x 2 FAT sectors, 112 root entries, 2-sector clusters | 354 | 3 + 531 | 362,496 | 40,960 (IBMBIO 5,120, IBMDOS 17,408, COMMAND 18,432) |
| /1 | 180 KiB, FCh, 2 x 2, 64, 1 | 351 | 3 + 527 | 179,712 | |
| /8 | 320 KiB, FFh, 2 x 1, 112, 2 | 315 | 3 + 473 | 322,560 | |
| /1/8 | 160 KiB, FEh, 2 x 1, 64, 1 | 313 | 3 + 470 | 160,256 | |

## The rewrite

`src/lib.rs` follows these routines with the same state, the same order of calls and output, and the same arithmetic (16-bit sizes, the IBM rounding of FNDSIZ and BAD100, DISP32BITS's BCD columns). Deliberate differences:

| ID | Difference | Why | Effect |
|---|---|---|---|
| D1 | The boot record is read at INIT from sector 0 of drive A (INT 25h), its OEM name set to `IBM  2.0` and its sector size to 512; DISKFORMAT puts the BPB in it as the original does. For /8 without /S, DONE does not write the original's DOS 1.10-style boot record (1450h): sector 0 keeps the 2.00 record BADSECTOR wrote | the boot records are IBM's code inside FORMAT.COM and may not be in this source; PC DOS 2.00 disk 1's boot sector is the original's 1650h record but for the OEM name (`PCJS.ORG` on the pcjs image) and the BPB, so it is the legal run-time source; the 1.10-style record exists only inside the original FORMAT.COM, which a run of the rewrite does not have (it takes that file's place) | identical boot records for 9-sector disks and for /8 with /S; for /8 without /S (f06, f07) sector 0 differs, and drive B is compared with sector 0 masked. A drive A whose boot sector is not PC DOS 2.00's (or unreadable) gives a different record |
| D2 | The FAT/directory buffer is 3,584 bytes in `.bss`; at INIT it is filled from `A:\COMMAND.COM` from byte 2410h (`LEFTOVER`, CS:1850h + 0BC0h), what the original's memory holds at its FATSPACE. `ASSUMPTION (spec D2)` in `src/lib.rs` | the original never clears what follows the FAT in its buffer, and that memory is COMMAND.COM's start-up image; the rewrite's own image covers that memory, so it reads the same bytes from the file | identical FAT sectors on PC DOS 2.00 with this COMMAND.COM loaded where the oracle's DOS loads it; without `A:\COMMAND.COM` the tail is zeros. A FAT longer than 3,581 bytes (not a diskette) is cleared only that far |
| D3 | INT 25h/26h, INT 11h and INT 13h are made by `src/bios.s` (`format_disk_io`, `format_equipment`, `format_bios_disk`; the last exchanges the sector IDs with 0060:0000 as 12CBh does) | the shared runtime has INT 21h only, and Rust has no inline assembly here | none |
| D4 | `/B` writes IBMBIO.COM (780h bytes) and IBMDOS.COM (1900h) both from the start of the system-file buffer | the original writes IBMBIO.COM from its own code at 39Eh, which may not be copied | the two files' bytes differ; not exercised |
| D5 | `build/format.com` is 8,925 bytes, the original 6,016 | llvm-i8086 code for the core | a disk holding the rewrite in place of FORMAT.COM has less free space; no case checks drive A |
| D6 | The system-file buffer starts at the end of `.bss` (3422h) instead of 3050h and ends 800h below the top of the program's memory (crt0_com.s's stack) instead of at 64 KiB with the stack at 0BF1h; the INT 23h handler is a Rust function doing FEXIT; INIT also reads drive A (D1, D2) | Rust layout | the memory left for the files is 3,041 bytes less (50,126 bytes) (no case is near it); the extra reads of drive A take time before the prompt, so on the Rust emulator the rewrite's label is stamped two seconds later than the original's (masked comparison) |
| D7 | The fixed-disk path (INIT's checks, DISKFORMAT's layout from IBMBIO's table at 0070:0000, BADSECTOR without formatting) is ported but not run | no fixed disk on the oracle machine | not exercised |

The labels' time stamp is DOS's clock at the create, not FORMAT's: it is compared masked where the machine's clock differs (plans/testplan.md).

Assumption ledger: `ASSUMPTION (spec D2)` in `src/lib.rs` is D2 above.
