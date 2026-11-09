# PRINT (PC DOS 2.00): behaviour specification

Derived from the original `PRINT.COM` (4,608 bytes, SHA-256 `a437d515...`, [../original/provenance.md](../original/provenance.md)) by reading it in full disassembled with GNU objdump 2.42 (`objdump -D -b binary -m i8086 -M intel --adjust-vma=0x100 PRINT.COM`, re-disassembled from 4FDh, 950h, 9BFh where data put the listing out of step). REA (spec/software-lifecycle.md) was not used: it needs a Ghidra/IDA/Hopper provider, none is installed here, and the program is about 2,300 bytes of code, read in full. IBM published no source, and PRINT is not in Microsoft's MIT-licensed MS-DOS 2.0 source release. Offsets are the `.COM`'s (CS:100h is the first byte). Observations are marked (O), derivations from them (D), inferences not run (I).

PRINT is a background print spooler. The first run asks for the list device, installs a resident part (INT 1Ch timer tick, INT 28h DOS idle, INT 2Fh queue requests, INT 13h, INT 17h or 14h, INT 5) and stays resident with INT 27h; each run then queues, cancels or lists files through INT 2Fh. The resident part reads each queued file with FCB sequential reads of 512 bytes and writes it one byte at a time straight to the list device's driver, expanding tabs, ending the file at the first ^Z (or the end of the file) and following each file with a form feed.

## Layout (O)

| Offset | Size | Use |
|---|---|---|
| 100h | 3 | `JMP 0D53h` (the transient part) |
| 103h-182h | | zeros: the resident part's own stack (SP = 183h) |
| 183h | 4 | far pointer to DOS's InDOS flag (function 34h) |
| 187h | 4 | the old INT 1Ch vector (offset, segment) |
| 18Bh | 1 | busy: the resident part is printing, or a BIOS disk call (INT 13h) is running |
| 18Ch | 1 | the slice was started by INT 28h (idle) |
| 18Dh, 18Eh | 1 each | ticks since the slice started; ticks since the last byte was sent |
| 18Fh | 1 | ticks to wait before the next timer slice (1 in the file, 8 after each slice) |
| 190h | 1 | an INT 2Fh request is being served |
| 191h | 4 | the old INT 28h vector |
| 195h | 1 | the `Canceled by operator` message has been printed in this request |
| 196h, 198h | 2 each | the interrupted SS and SP |
| 19Ah | 4 | the DTA saved around a read |
| 19Eh | 4 | the INT 24h vector saved around a read |
| 1A2h | 4 | far pointer to the list device's driver header |
| 1A6h | 1 | the print column (for tabs) |
| 1A7h | 2 | the next byte of the buffer; 4FCh: the buffer is empty (the next slice reads a record) |
| 1A9h | 2 | the head of the queue: the entry being printed |
| 1ABh, 1ADh | 2 each | the last entry and the previous entry while a cancel walks the queue |
| 1AFh | 1 | a critical error happened during the read |
| 1B0h-2FBh | | the messages printed to the list device (below) and the INT 24h message table (233h: 13 words) |
| 2FCh-4FBh | 512 | the record buffer (the DTA of the reads) |
| 4FDh | | INT 1Ch; 557h INT 28h; 585h the print slice; 6C8h INT 24h; 73Eh INT 2Fh; 8FBh next file; 915h, 920h, 950h, 978h the device routines |
| 935h | 1 | the byte written; 936h far pointer used to call the driver's strategy and interrupt routines |
| 93Ah | 22 | the request header: length 16h, command (93Ch: 8 write, 0Ah output status), status word 93Dh, transfer address 948h:94Ah = CS:935h, count 94Ch = 1 |
| 9B9h, 9BFh | | the old INT 13h vector; the INT 13h hook |
| 9E0h, 9E4h, 9E8h, 9EAh | | the old INT 5 and INT 17h vectors; the printer number (0-2); the INT 17h hook |
| A11h, A15h, A17h | | the old INT 14h vector; the serial port number (0-1); the INT 14h hook |
| A49h | | the INT 5 hook |
| A66h | 2 | the number of queue entries, 10 |
| A68h | 2 | device errors: status calls since the device was last ready |
| A6Ah-BF9h | 10 × 28h | the queue: a ring of entries, each a word (the next entry) and 38 bytes (an opened FCB, from its drive byte); drive byte FFh: the entry is free. In the file every entry is free and links to the next, the last to A6Ah |
| BFAh-11C3h | | the transient part (from D53h), its messages and its tables; INT 27h keeps CS:0000h-0BF9h |
| 11C4h, 11EDh | | the device tables: count 8, name, number: `PRN` 0, `LPT1` 0, `LPT2` 1, `LPT3` 2, end 0; `AUX` 0, `COM1` 0, `COM2` 1, end 0 |
| 120Ch, 121Ch | | the function 0Ah buffer (14 bytes); the list device's FCB (drive 0, `PRN` and 8 spaces) |

## The transient part (D53h)

- (O) Function 30h with AL and AH swapped: below 0136h or above 0200h is `Incorrect DOS version` CR LF, INT 20h.
- (O) Function 3700h: the switch character (DL) is kept.
- (O) The 13 bytes at the INT 2Fh vector (function 35h) are compared with this copy's 73Eh-74Ah. Not equal: the install (below), which asks for the device first. Equal: the resident copy is used as it is.
- (O) Function 19h: the default drive.
- (O) The command tail (PSP:80h): length 0 goes straight to the status. Otherwise tokens are read from 81h:
  - blanks are space, tab, `,` and `=`; CR ends the line;
  - the switch character followed (after blanks) by one character is a switch; CR there ends the line;
  - anything else is a name, up to a blank, CR or the switch character.
- (O) A name is parsed with function 2901h (leading separators skipped) into the FCB at 5Ch; drive 0 is made the default drive + 1. It is kept *pending*; the name before it, if one is pending, is processed first.
- (O) Switches: `C`/`c` sets cancel mode, `P`/`p` print mode (the start), `T`/`t` cancels every file (2Fh AH=1 DX=FFFFh), drops the pending name and ends the line (status, exit). Any other: `Invalid parameter` CR LF; the parse goes on.
- (O) At CR a pending name is processed. So a switch applies to the name before it as well as to those after (`print a /c` cancels A; `print /c a b /p c` cancels A and queues B and C).
- (O) Processing a name (E44h), with the mode at that moment:
  - FCB name blank: `X:NAME    .EXT  File not found` CR LF (the drive letter and the name filled into the template ` :        .     File not found`). The same message when the search finds nothing.
  - cancel mode: 2Fh AH=1 with DS:DX the FCB at 5Ch (its `?` match any character).
  - print mode: DTA at FF2h, function 11h on the FCB; for each entry found: function 0Fh opens the found FCB at FF2h; failing: `Cannot open X:NAME    .EXT ` CR LF; else 2Fh AH=0 with DS:DX = FF2h; a result of AL ≠ 0 the first time prints `PRINT queue is full` CR LF. Then DTA FF2h again, function 12h (search next) on 5Ch.
- (O) The status (EF2h): 2Fh AH=FFh. If the word before ES:BX (A68h) is 1000 or more: `Errors on list device indicate that it` CR LF `may be off-line. Please check it.` CR LF CR LF. AH = 0: `PRINT queue is empty` CR LF. Otherwise, from ES:DX: CR LF, 8 spaces, `X:NAME    .EXT is currently being printed` CR LF, and for each of the next AH-1 entries (following the links) 8 spaces, `X:NAME    .EXT is in queue` CR LF. The drive letter is the FCB's drive byte + 40h, or the default drive's letter for 0.
- (O) End: if this run installed the resident part, INT 27h with DX = 0BFAh; else INT 20h. The status is shown on every run, after the tail.
- (D) The cases show the order: the device question, `Resident part of PRINT installed`, errors of the tail (`Invalid parameter`, `File not found`), the status.

## The install (C03h)

- (O) `Name of list device [PRN]: `, function 0Ah (14 bytes), CR LF. The typed characters (if any) are copied over the FCB's name at 121Dh as typed (no padding: `lpt1` leaves `lpt1` and the 7 spaces after `PRN`). Function 0Fh on the FCB; failing, or byte 18h of the FCB without bit 7 (not a device): `List output is not assigned to a device` CR LF, INT 20h.
- (O) FCB bytes 19h-1Ch: the device driver's header (far pointer) to 1A2h; its segment also to 938h. 94Ah = CS.
- (O) INT 28h → 557h (old kept), INT 2Fh → 73Eh (old not kept), INT 13h → 9BFh (old kept); the INT 17h, 14h and 5 vectors are kept.
- (O) The 8 name bytes made capital; looked up in the LPT table: found, 9E8h = its number, INT 17h → 9EAh, INT 5 → A49h. Else in the COM table: found, A15h = its number, INT 14h → A17h. Else no hook.
- (O) Function 34h: the InDOS pointer to 183h. INT 1Ch → 4FDh (old kept). `Resident part of PRINT installed` CR LF.

## The resident part

### INT 1Ch (4FDh)

- (O) 18Dh and 18Eh are incremented. If 18Fh ≠ 0 it is decremented; else if busy (18Bh) or InDOS ≠ 0 nothing more; else busy is set, 18Dh = 18Eh = 0, interrupts enabled, EOI (20h to port 20h), the print slice, interrupts disabled, 18Fh = 8, busy cleared. Then `JMP FAR` to the old INT 1Ch.
- (D) A slice runs at most every ninth tick, and only outside DOS. The tick's own EOI is given before the slice, so further ticks (which only count) arrive during it.

### INT 28h (557h)

- (O) If not busy: busy set, 18Ch set, interrupts enabled, the slice, interrupts disabled, 18Ch = 0, 18Fh = 8, busy cleared. Then the old INT 28h.

### The print slice (585h)

- (O) Nothing if the head entry is free. Otherwise AX pushed, SS:SP saved and moved to CS:183h, the other registers pushed, DS = CS.
- (O) If the buffer is empty (1A7h = 4FCh), a read (below), then the slice ends.
- (O) Otherwise, until the slice ends:
  1. buffer exhausted (1A7h = 4FCh): end;
  2. a timer slice (18Ch = 0) with 18Dh ≥ 2 (two ticks spent): end;
  3. the device's status (950h); not ready: in an idle slice end; in a timer slice end if a tick has passed since the last byte (18Eh ≥ 1), else back to 1;
  4. the byte at 1A7h: ^Z is the end of the file: a form feed (920h) and the next file, end. CR sets the column to 0; TAB is written as spaces up to the next multiple of 8 (8 - column mod 8 spaces, the column advanced); BS decrements the column; another byte is written (978h), the column advanced if it is 20h or above;
  5. 1A7h incremented, 18Eh = 0; an idle slice ends after one byte, a timer slice goes back to 1.
- (D) The bytes printed for a file are its bytes up to the first ^Z, tabs expanded, then a form feed. The column counts bytes from 20h up since the last CR (an LF does not reset it).

### The read (659h)

- (O) INT 24h → 6C8h and the DTA → 2FCh (both saved first), 1AFh = 0, function 14h (sequential read) on the head entry's FCB (512-byte records, set when it was queued), then the DTA and INT 24h put back. A critical error (1AFh ≠ 0): the next file (no form feed). AL = 1 (no data: end of file): a form feed and the next file. Otherwise (0, or 3 with the rest of the record zeros) 1A7h = 2FCh.
- (D) A file without ^Z prints the zeros that pad its last record, then (at the next read) the form feed.

### Next file (8FBh)

- (O) The head entry is made free, the head moves to its next, the buffer is empty, the column 0.

### INT 24h during a read (6C8h)

- (O) If 1AFh is already set: AL = 0, IRET. Otherwise the drive (AL) is added to the `A` of `File allocation table bad drive A.` (2F7h, never put back), the list device gets CR LF CR LF `**********` CR LF and then, for AH bit 7, `File allocation table bad drive X.` CR LF, else the message of the error code (DI, at most 0Ch, from the table at 233h: `Write protect`, `Bad unit`, `Not ready`, `Bad command`, `Data`, `Bad call format`, `Seek`, `Non-DOS disk`, `Sector not found`, `No paper`, `Write fault`, `Read fault`, `Disk`), then ` error reading file` CR LF, `X:NAME    .EXT` CR FF BEL (the head entry's FCB). 1AFh = 1, AL = 0 (ignore), IRET.

### INT 2Fh (73Eh)

- (O) If a request is already being served (190h): AX = FFFFh, IRET. Otherwise 190h set, interrupts enabled, 195h = 0, then by AH (AL is not looked at; nothing is chained):
  - AH = 0, submit DS:DX: waits until not busy, sets busy; the first free entry from the head (following the links, at most 10); none: AL = 1. Else the 38 bytes at DS:DX are copied into it, its FCB's current block and current record set to 0 and record size to 512; AL = 0.
  - AH = 1, cancel: waits until not busy, sets busy. DX = FFFFh: every entry is made free; if the head was in use the list device gets CR LF CR LF `All files canceled by operator` CR FF BEL; the buffer is emptied. Otherwise DX is an FCB in DS: from the head, each entry in use whose drive byte equals the FCB's and whose 11 name bytes match (the FCB's `?` matches anything, letters compared in capitals) is made free. The head: the list device gets (the first time in the request) CR LF CR LF `X:NAME    .EXT Canceled by operator` CR FF BEL, and the next file. Another entry is moved behind the queue's last entry (so the entries in use stay together from the head). AL = 0.
  - any other AH: AL = 0.
  - Then (all three): AH = the number of entries in use, ES:BX = A6Ah (the queue), DX = the head; it waits until not busy and makes a status call to the device (950h); 190h = 0, IRET.

### The device routines

- (O) 978h write: AL to 935h, command 8, status 0, count 1, ES:BX = the request header, `CALL FAR` the driver's strategy routine (header word 6) and then its interrupt routine (word 8).
- (O) 950h status: A68h incremented; command 0Ah (output status) the same way; a status with bit 15 (error) gets bit 9 (busy) set; not busy: A68h = 0. ZF = 1 when ready.
- (O) 920h write with wait: status until ready, then write; while not ready, once A68h is above 20000 the byte is dropped. 915h writes a `$`-terminated string with 920h.
- (D) PRINT does not go through DOS or INT 17h to print: it calls the driver itself, and the driver of `PRN`/`LPTn` calls INT 17h.

### INT 13h, 17h, 14h, 5 (9BFh, 9EAh, A17h, A49h)

- (O) INT 13h: busy incremented, the old handler called (as an interrupt, returning to 9D6h), busy decremented, `RETF 2` (the flags of the BIOS kept).
- (O) INT 17h: if the head entry is in use, DX = 9E8h and not busy: AH = A1h, IRET (another program's printer call fails while PRINT prints); else the old handler. PRINT's own driver calls pass, since they run while busy.
- (O) INT 14h: likewise with A15h: AH = 80h (time-out), AL = 0 for AH = 0 or above 2.
- (O) INT 5 (Print Screen): if the head entry is in use and 9E8h = 0: IRET; else the old handler.

## The rewrite

`src/lib.rs` has the transient part (`run`) over a `Dos` trait and `resident`, the resident part's behaviour above in Rust over the resident copy's bytes, which the host replay (`tests/oracle_replay.rs`) runs in front of its model of DOS. `src/resident.s` is the resident part itself, in assembly, with the data at the original's offsets; `src/main.rs` is the target's `Dos`. Deliberate differences:

| ID | Difference | Why | Effect |
|---|---|---|---|
| D1 | The resident part (the interrupt handlers, the slice, the read, the queue requests, the device calls) is assembly (`src/resident.s`), written from this specification, not Rust | the handlers run on the interrupted program's stack (the slice on its own at CS:183h), must keep every register and flag, chain with `JMP FAR`, call the driver with `CALL FAR` and must stay in the first 0BFAh bytes at fixed offsets; compiled Rust needs DS = SS = CS, a frame and the runtime (no inline assembly) | the same behaviour; `resident` in `lib.rs` is the same logic in Rust for the host |
| D2 | The data are at the original's offsets (100h-4FBh, the queue at A66h-BF9h, the messages at 1B0h-2FBh), but the code from 4FCh to A65h is the rewrite's own, and so are the offsets of its variables there (the request header at 4FDh, the byte written at 4FCh, the driver call pointer at 513h, the saved INT 13h/17h/14h/5 vectors at 517h-526h, the printer and port numbers at 527h, 529h) and of its handlers (INT 1Ch 52Bh, 28h 587h, 2Fh 787h, 13h 9D3h, 17h 9E8h, 14h A0Eh, 5 A3Fh) | the code was not copied | none visible. The INT 2Fh handler's first 13 bytes (the test of 190h, the jump to the busy reply just before it, the setting of 190h), which a later run compares, come out byte for byte as the original's, so (I: not run) the original's transient part would use the rewrite's resident copy and the other way round: the requests, the replies and the queue entries are the same. No case mixes them |
| D3 | The transient part (from BFAh) is Rust with its own start (`resident.s`), the stack at the top of the program's memory as `crt0_com.s` sets it; its buffers (the tail's token, the device FCB, the 0Ah buffer, the found FCB) are Rust locals, not at 1241h, 121Ch, 120Ch, FF2h | Rust | none visible: INT 27h keeps the same 0BFAh bytes, the queue entries hold the same 38 bytes |

## Assumption ledger

The rewrite's code makes no assumption; these are the host replay's model (`tests/oracle_replay.rs`), not of PRINT:

| ID | Assumption |
|---|---|
| H1 | DOS 2.00's FCB calls as the cases use them: 11h/12h search the root of drive A by an FCB pattern (`?`), and a device name (`PRN`, `LPT1`-`3`, `AUX`, `COM1`-`2`, `CON`, `NUL`) is found as itself; 0Fh, comparing the name in capitals, opens a file (record size 128, the size, bit 7 of byte 18h clear) or a device (bit 7 set, bytes 19h-1Ch the driver); 14h on a device returns AL = 1 (no data); 2901h skips leading separators (`:.;,=+`, space, tab) and stops a name at a separator, `/`, `"`, `[`, `]`, `<`, `>`, `|` or a control character, making letters capital and `*` into `?` |
| H2 | COMMAND echoes the command line, runs PRINT, then shows CR LF and `A>`; function 0Ah echoes the typed characters and CR (no LF); the console draws CR, LF and characters 20h-7Eh |
| H3 | Background printing on the host: after PRINT has stayed resident the model calls the INT 1Ch handler's logic tick by tick (InDOS 0, the device always ready, no tick counted during a slice) until the queue is empty; the order of the bytes does not depend on when the slices run |
| H4 | The printed lines (`#lpt|`) are what 86Box's Generic Text Printer (`src/printer/prt_text.c` of 86Box `a1e92633`) writes for the bytes, with no INIT pulse after the first page and no 5-second pause inside the stream (`tests/common/mod.rs`, `render_text_printer`) |
