# FORMAT: I/O contract

| Channel | Call | Use |
|---|---|---|
| DOS version | INT 21h AH=30h | AH.AL swapped must be 2.00 or later, else `Incorrect DOS version` (09h), INT 20h |
| ASSIGN check | 0000:0084h (INT 21h vector), xxxx:0103h-010Ah | ASSIGN.COM's handler at xxxx:0122h with a drive table that is not 1-8: `Cannot format an ASSIGNed drive. `. The rewrite reads them with `ibm_pc_dos::far_read16` |
| Drive check | AX at entry | AL = FFh: `Invalid drive specification`, INT 20h |
| Default drive | INT 21h AH=19h | DEFALT and the drive the system files come from; the target is FCB 1's drive (PSP:5Ch) or the default |
| Ctrl-Break | INT 21h AX=2523h | INT 23h: FEXIT (CHDIR back with /S, INT 20h) |
| Switch character | INT 21h AX=3700h | DL: the switch character in the tail (PSP:80h/81h) and in ROOTSTR (`B:/`) |
| Disk reset | INT 21h AH=0Dh | in CLEAR (twice) and before DONE |
| Directories | INT 21h AH=3Bh | the default drive's root (/S), the target's root (CLEAR), USERDIR at the end (/S) |
| Drive parameter block | INT 21h AH=32h | the target's clusters (+0Dh - 1), cluster size (+4 + 1), sector size (+2) in CLEAR; the default drive's current directory (+1Ch, +1Eh text) in SAVUDIRS. The rewrite reads it with far accesses |
| FCB functions | INT 21h AH=0Fh, 10h, 13h, 16h | open IBMDOS.COM, IBMBIO.COM (extended, attribute 07h) and COMMAND.COM on the default drive; delete `???????????` (extended, attribute FFh) on the target; create and close the system files and the volume label (extended, attribute 08h) |
| DTA, block I/O | INT 21h AH=1Ah, 27h, 28h | the system-file buffer (the original: CS:3050h; the rewrite: the end of `.bss`, spec D6), record size 1, CX = bytes |
| Console | INT 21h AH=09h, 02h, 0Ah, AX=0C08h, 0C01h | messages, digits; the label line (80 characters); `strike any key` (0C08h, no echo); `Format another (Y/N)?` (0C01h, echoed) |
| Sectors | INT 25h, INT 26h (AL drive, DS:BX, CX count, DX sector) | write: the FAT to both copies (WRTFAT, three times a disk), the boot record to sector 0, DONE's directory (/8); read: DONE's directory (/8). The rewrite also reads sector 0 of drive A at INIT (spec D1). Both leave the flags on the stack; `src/bios.s` removes them (spec D3) |
| Equipment | INT 11h | bits 7-6: the diskette drives (INIT, SYSPRM) |
| Diskette BIOS | INT 13h AH=00h, 04h, 05h, 08h | reset (AL = sectors per track, DL = drive); format (AL = sectors per track, CH = track, DH = head, the nine C/H/R/N IDs moved to 0060:0000, ES:BX = 0060:0000, then moved back); verify (CH = track, CL = 1, AL = sectors per track); AH=08h DL=80h for the fixed disks. Errors 03h and 80h end the format with a message, others are retried three times |
| DOS's drive byte | 0050:0004h | the target drive (diskettes) |
| Files of drive A (rewrite only) | INT 21h AH=3Dh, 42h, 3Fh, 3Eh | 3,584 bytes of `A:\COMMAND.COM` from 2410h into the FAT buffer at INIT (spec D2) |
| Exit | INT 20h | no exit code |

The target build (`src/main.rs`) is a `.COM` (`crt0_com.s`, `link_com.ld`, `i8086-ld --com`): 8,925 bytes of program (7,946 code, 979 read-only data), 4,165 bytes of `.bss` (the 3,584-byte FAT/directory buffer, the 512-byte boot record, the Ctrl-Break state) to 3422h; the system-file buffer from 3422h to 800h below the top of the program's memory (at most 0FFF0h), the stack above it.
