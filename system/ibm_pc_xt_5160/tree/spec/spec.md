# TREE (PC DOS 2.00): behaviour specification

Derived from the original `TREE.COM` (1,513 bytes, SHA-256 `132d9995...`, [../original/provenance.md](../original/provenance.md)) by reading it in full disassembled with GNU objdump 2.42 (`objdump -D -b binary -m i8086 -M intel --adjust-vma=0x100 TREE.COM`). REA (spec/software-lifecycle.md) was not used: it needs a Ghidra/IDA/Hopper provider, none is installed, and the program is small enough to read in full. IBM published no source. Its copyright string reads `The IBM Personal Computer Directory Path Display Utility Version 1.00 (C)Copyright IBM Corp 1983 ... Author - David Litton`. Offsets are the `.COM`'s (CS:100h is the first byte). Observations are marked (O), derivations from them (D), inferences not yet run (I).

## Memory (O)

| Offset | Size | Use |
|---|---|---|
| 3F2h | word | end of the path (its NUL) |
| 3F4h | word | the separator before the path's last component |
| 3F6h | word | start of the name list |
| 3F8h | byte | flags: 01h list overflow, 02h list title printed, 04h `Path:` printed, 08h files pass, 10h `/F`, 40h a name already printed (indent the next), 80h the directory has a subdirectory |
| 3F9h | byte | the default drive at entry (function 19h) |
| 3FAh | word | names per list chunk, minus one |
| 3FCh | 2 | `\` NUL: the root, for CHDIR |
| 3FEh | 44 | the DTA (function 1Ah): the extended FCB of the entry found; the name at 406h (DTA+8), its attribute at 411h (DTA+19) |
| 426h | 44 | extended FCB 1: attribute at 42Ch (08h, then 10h: directories), drive 0 (default), name 42Eh `???????????` |
| 452h | 44 | extended FCB 2: attribute 0 (normal files), name `???????????` |
| 523h | byte | the path separator, `\` (`/` if the switch character is not `/`) |
| 524h | 64 | the target drive's current directory (function 47h), so 523h holds `\` + it |
| 564h | to PSP:0006h | the path (NUL-terminated), then the name list, 11 bytes per name; this overwrites the initialisation code at 565h, which has run by then |

The stack is moved to SP = 100h (103h), into the PSP, so the list may grow to PSP:0006h.

## Initialisation (565h-5F4h)

- (O) 567h: function 30h; unless the major version is exactly 2, prints `\r\nIncorrect DOS version\r\n\a` with function 09h and ends with INT 20h.
- (O) 578h: if AL or AH at entry is FFh (COMMAND found an invalid drive in the first or second parameter), writes `\r\nInvalid drive specification\r\n\a` (32 bytes) to handle 2 and ends (INT 20h).
- (O) 590h: saves the default drive (19h); flags = 0; 59Ch: switch character (3700h); if not `/`, the separator becomes `/`.
- (O) 5ACh: if the tail (PSP:80h length, PSP:81h text) holds the switch character (`repnz scasb` over the length, CR excluded), the character after it, ORed with 20h, must be `f` (flag 10h); anything else, or the switch character as the tail's last character, writes `\r\nInvalid parameter\r\n\a` (22 bytes) to handle 2 and ends. Only the first switch character is looked at (D: `/f/x` is `/F`; `b:/f` works).
- (O) 5CDh: INT 23h (Ctrl-Break) vector to 13Ah. 5D5h: if the FCB 1 drive (PSP:5Ch) is not 0, selects that drive (0Eh). 5E2h: function 47h for that drive (0: default) into 524h. 5EDh: DTA = 3FEh.

## Main (100h-1CDh)

- (O) 106h: CHDIR `\`. 109h: search first (11h) with FCB 1, attribute 08h: the volume label, or, if none is found, FCB 1's own `???????????`, goes into the header. Then FCB 1 gets attribute 10h for the rest of the run.
- (O) 12Ch: the subdirectory search (below) in the root; if it finds none, writes `\r\nNo sub-directories exist\r\n\a` (29 bytes) to handle 2 and exits (13Ah).
- (O) 13Fh: path = empty; writes to handle 1 the header `\r\nDIRECTORY PATH LISTING FOR VOLUME ` + the 11-byte label + `\r\n\n\n` (51 bytes).
- (O) The walk, depth first, every subdirectory in directory order before its next sibling (D: preorder); the root itself gets no block:
  - 18Ah (descend): append the separator and the found name (`NAME` or `NAME.EXT`, spaces dropped) to the path; CHDIR to it. If CHDIR fails: `Path: ` + path + `\r\n\n` and `Invalid path\r\n\n\n\n\a` to handle 2, then go up. Otherwise list the directory (1CEh), reset FCB 1's name to `?`s, and if the listing found a subdirectory (flag 80h), search it for its first subdirectory and descend; else go up.
  - 160h (up): FCB 1's name = the path's last component; drop it from the path (3BEh); CHDIR to the parent (`\` if the path is now empty); search first for that name (positioning FCB 1 at the entry in the parent), then, with the name back to `?`s, search next: a further subdirectory is descended into; none, and the path not empty, goes up again; none at the root exits.
- (O) Exit (13Ah, also the Ctrl-Break handler): CHDIR to 523h (separator + the directory saved at start, on the target drive), select the saved default drive (0Eh), INT 20h.

## The subdirectory search (2B2h-2DDh)

- (O) Search first/next (11h/12h) with FCB 1 (attribute 10h, name `???????????`, or a name at 175h); skips entries whose attribute lacks 10h (DOS also returns normal files to attribute 10h), has 02h or 04h (hidden, system), or whose name starts with `.` (`.`, `..`); a hit sets flag 80h.
- (O) The file search (2DEh-2EBh): search first/next with FCB 2 (attribute 0: DOS returns normal and read-only files, not hidden, system, directories or the label); no filter.

## Listing a directory (1CEh-2B1h)

- (O) flags &= 10h. 1D3h: chunk size = (PSP:0006h - (path end + 1)) / 11, minus one; the list starts after the path's NUL.
- (O) Search first (directories, or files in the files pass), then next, storing each 11-byte name; when the chunk is full, flag 01h.
- (O) The first time: `Path: ` + path + `\r\n\n` (flag 04h); then the title (flag 02h) `Sub-directories:  ` or, in the files pass, `Files:            ` (18 bytes each); then the names (2ECh): `None\r\n` if there are none, else per name the 8-byte name, the extension as `.EXT` (or 4 spaces if it is blank: the `.` or space is written over the name's 8th byte after the name is printed), `\r\n`; every name after the first in the pass is preceded by 18 spaces (flag 40h).
- (O) After the names: `\r\n`. With `/F`, after the directories pass, the files pass (flags &= 90h, |= 0Ch: the path is not printed again). Then `\n\n`.
- (D) So a directory's block is: `Path: \A\B`, an empty row, `Sub-directories:  X` (more names below it, indented), an empty row, [with `/F`: `Files:            F.EXT` ..., an empty row,] then two empty rows.
- (O, I) Overflow (272h): with flag 01h the next chunk is read into the same list (at 20Bh: search next first) and printed indented, with no title; flag 01h is never cleared within the pass, so after the last chunk the loop continues: search next fails, an empty list prints `None` (indented) and repeats (I: the original then prints `None` forever; it needs more than chunk-size names in one directory, about 5,800 with a 64 KiB segment; not run).

## Paths (389h, 3BEh, 35Fh)

- (O) The path grows only in TREE's buffer; its depth is limited by DOS: when CHDIR refuses a path (DOS 2.00 keeps a current directory of at most 63 characters, I), the `Invalid path` block is shown and the subtree skipped.
- (O) 3BEh: dropping the last component writes NUL at the separator (3F4h), then, unless that was the first byte, finds the previous separator with a backward `repnz scasb` of 12 bytes from the byte before it. (D) A component of 12 characters (`ABCDEFGH.EXT`) is not crossed: 3F4h then points at its first character instead of the separator before it, and the next level up searches the parent for the name without that first character (I: not run; DOS's answer to that search decides what follows).
- (O) 35Fh: the name for FCB 1 is the path from 3F4h+1 to the NUL, `.` moving to the extension.

## Output (O)

Everything except the version message (09h) is written with function 40h: the header, the blocks to handle 1; `No sub-directories exist`, `Invalid path` and the parameter and drive errors to handle 2. Messages ending with BEL (07h) sound the speaker.

## The rewrite

`src/lib.rs` follows these steps with the same buffers (path and list in one byte buffer, the three 44-byte FCB/DTA records). Deliberate differences:

| ID | Difference | Why | Effect |
|---|---|---|---|
| D1 | The path and the name list are in a 16,384-byte static buffer; the chunk size is computed from its end, not from PSP:0006h | Rust objects are limited to 32,767 bytes with 16-bit pointers, and the stack stays at the top of the segment (the original moves it into the PSP) | about 1,480 names per chunk instead of about 5,800: the overflow path of the original (and its endless `None`, see Listing) starts at a smaller directory; no case reaches it |
| D2 | The version message is printed with function 02h, byte by byte, not 09h | no `$`-terminated string | the same characters |
| D3 | The DTA is set (1Ah) before each search, not once | the DTA lives in the core's state, not at a fixed address | none visible |
| D4 | The Ctrl-Break handler is a Rust function that restores the directory and the drive and ends with INT 20h, as 13Ah does | no code at a fixed address | the same effect; not exercised by a case |
