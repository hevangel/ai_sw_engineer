# EDLIN: I/O contract

Every pointer below is an offset in the program segment prefix's segment (the original's own segment, DG); the
rewrite passes it with DS = ES = that segment (spec D1, D2).

| Channel | Call | Use |
|---|---|---|
| Drive check | AX at entry | AL = FFh (COMMAND found an invalid drive in the first parameter): `Invalid drive or file name` (for a name without a directory) |
| DOS version | INT 21h AH=30h | AL below 2: `Incorrect DOS version`, INT 20h |
| Switch character | AX=3700h | DL = `/`: options with `/`, directories with `\` |
| PSP | 06h, 5Ch, 81h | word 6 - 1 is LAST, the end of the text; FCB 1 (COMMAND's parse of the name, or 2901h's of its name part) is the file, opened, read and renamed in place; the tail holds the name and `/B` |
| Directory | AH=19h, 0Eh, 1Fh, 3Bh | only for a name with a directory: the drive kept, the name's drive selected, the drive's current directory read from the DPB (DS:BX, 1Ch and 1Eh), CHDIR to the name's directory; REST_DIR at the end: CHDIR back, the drive selected again |
| Parse | AX=2901h | the name part (with a directory), the file of T (into FCB3, 1466h) |
| Files | AH=0Fh, 10h, 13h, 16h, 17h | open the file (or create, close, delete it again for a new file); NAME.$$$ (FCB2, 1441h) created, closed, deleted on Q; NAME.BAK deleted at the first write (13h with FCB2 holding `BAK`); at E NAME renamed NAME.BAK and NAME.$$$ renamed NAME |
| Read | AH=1Ah, 27h | random block reads, record size 1, into the text at its end (3/4 of memory at the start, the free memory for A and E, the gap for T); AL = 1 is the end of the file |
| Write | AH=1Ah, 28h | random block writes of NAME.$$$ from START (W, E), then the one byte at ENDTXT (E) |
| Console | AH=02h | every character EDLIN prints itself: the prompts, line numbers, lines, CR and LF |
| | AH=09h | the messages (`$`-terminated) |
| | AH=0Ah | the command line into COMBUF (12BCh, 128 bytes); a line into EDITBUF (133Eh, 255 bytes) for I and for an edited line (with the line as the template) |
| | AH=01h | the key after `O.K.? ` and `Abort edit (Y/N)? ` (echoed; no type-ahead flush) |
| Ctrl-C | AX=2523h | INT 23h to ABORTCOM at each prompt, to ABORTINS for I, ABORTMERGE for T; END_INS sets ABORTCOM again |
| Critical errors | AX=3524h, 2524h | INT 24h chained to the old handler (only with a directory), restored by REST_DIR; Abort ends EDLIN after REST_DIR |
| Exit | INT 20h | no exit code (the rewrite: with CS = the PSP's segment, a far jump to PSP:0000) |

The target build (`src/main.rs`) is linked at 100h with `link_com.ld`, the shared `runtime/dos.s`, `src/start.s`
in place of `crt0_com.s` and `src/edlin.s`, and wrapped by `scripts/mkexe.py` into a load-high `.EXE` shipped as
EDLIN.COM: a 32-byte MZ header (no relocations, MINALLOC = MAXALLOC = 0, CS = SS = FFF0h, IP = 100h, SP = the end of
the stack) and the image with `.bss` (10 bytes) and a 2 KiB stack as zeros. `src/edlin.s` has `edlin_dos` (INT 21h
from a register block, DS from the block, ES = the PSP's segment; 1 if Ctrl-C ended it), `edlin_int23` (resumes out
of that call), `edlin_int24` (the original's INT_24 chain), `edlin_set23`, `edlin_hook24`, `edlin_unhook24`,
`edlin_exit`, and `edlin_peek`, `edlin_poke`, `edlin_far_byte`, `edlin_movsb`, `edlin_scasb`, `edlin_cmpsb` on the
PSP's segment. The program is 12,156 bytes: code 9,649 bytes, read-only data 415 (the messages), data 2.

## Memory map on the oracle machine (256 KiB, PSP at 0600h)

| Paragraphs | Original | Rewrite |
|---|---|---|
| 0544h-0993h at the boot | COMMAND.COM's file (17,664 bytes; its PSP at 0534h), loaded by DOS; its bytes stay where the program and text later lie | the same |
| 0600h-060Fh | PSP | PSP |
| 0610h-072Fh | the file (4,608 bytes): code, copyright, constants, messages, the first variables | not touched: what the boot left (COMMAND.COM's bytes) |
| 0730h-0753h | the variables, COMBUF, EDITBUF, FCB2, FCB3, the stack (to 1539h) | COMBUF (12BDh-12FFh set to the original's file bytes), EDITBUF, EOL, FCB2, FCB3 at the original's offsets |
| 0753h:0009h (1539h) to 0600h:FFEFh | START to LAST: the text, the gaps of I and T, the copy of M and C at the top | the same |
| 3CB6h-3FABh | COMMAND's transient part | the rewrite: image, `.bss`, stack (12,124 bytes), placed by COMMAND's EXEC for a load-high `.EXE` at the top, just below the EXEC code itself (3CB6h:0000h, observed on the Rust emulator); that is memory of COMMAND's transient part, which COMMAND checks after a program and reads from COMMAND.COM again if it was overwritten (I) |
| 4000h | PSP:0002h, the top of memory | the same |
