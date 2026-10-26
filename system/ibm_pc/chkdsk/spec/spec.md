# chkdsk specification

Behavior of IBM PC DOS 1.00 `CHKDSK.COM` (1395 bytes, loaded at CS:0100h), derived from a disassembly of the original. It becomes a verified specification only when each statement is replayed against the original's output on the reference machine ([oracle](../oracle/README.md)); until then every claim here is **read from the code, not yet observed** (see A1).

## Sources

- The original binary ([provenance](../original/provenance.md)); addresses below are offsets in the loaded COM image (file offset + 100h).
- The PC DOS 1.00 function-call interface, as seen through the INT 21h calls the program makes.
- Disk layout: the module documentation of [`pcdisk`](../../platform/tools/pcdisk/src/lib.rs), read off the real DOS disk.

## Behavior (as read from the code)

### Command line (100h–14Ah)

- `SP` is set to 782h: the program owns a stack and a work area above its image, so image, FAT and the table of directory records share one 64 KiB segment.
- `AL` at entry is the "first FCB drive invalid" flag. Non-zero gives *Invalid drive specification*. The first byte of the second FCB (5Dh) must be a space, otherwise *Invalid parameter*.
- The current drive is saved (AH=19h) in 681h. If a drive was given it is selected (AH=0Eh) and compared with the drive count returned in AL; a drive at or above the count gives *Invalid drive specification*.
- Vector 24h (critical error) is replaced by a handler at 3BBh around the checks (385h–3BAh save and restore the vector through the interrupt table at 90h). The handler runs STI and, for the checked drive on a disk error, returns "ignore"; otherwise it restores the current drive and chains to the old handler.

### Reading the FAT and the directory (14Eh–1B0h)

1. The FAT information call (AH=1Bh, helper at 3EDh) returns the FAT in memory (`ES:BX`), sectors per cluster (`AL`), number of clusters (`DX`) and sector size (`CX`). The first FAT byte must be FEh, otherwise *Diskette not initialized*.
2. The DTA (AH=1Ah) is set to the work area at 782h, and the directory is walked with search-first/next (AH=11h/12h) on `????????.???` (the FCB at 4FEh). Each match is kept as a 2Ch-byte record; the record count is stored in 677h.
3. The FAT is held in memory and read as 12-bit entries (routine 408h reads entry `SI`, 424h writes it; entry *n* is at byte *n* + *n*/2, odd entries use the high 12 bits).

### Checking every file (1C3h–2F0h)

For every directory record, follow the cluster chain from its first cluster:

- Every cluster visited is marked in an owner table (at 77Eh, indexed by cluster). A cluster that already has an owner reports *Files cross-linked:* with the two file names (4D3h prints a record's 8-character name, a space and the 3-character extension).
- A chain entry beyond the last cluster, or a free entry inside a chain, is an *Allocation error for file* NAME; the chain is cut with end-of-chain FFFh (424h). Values FF8h and above end a chain normally.
- The chain length is converted to bytes (clusters × sectors per cluster × sector size) and compared with the directory entry's size. A difference prints *File size error for file* NAME and the program rewrites the entry through open and close (AH=0Fh, 10h, with random record AH=28h) with the size taken from the chain, writing the disk. **The original therefore modifies the disk when it finds errors.**
- A failed directory search or file operation prints *Directory error-file:* NAME.

### Lost chains and totals (2F3h–384h)

- A cluster that the FAT marks used but no directory record owns is counted, and the program sets its FAT entry back to free (424h with value 0). If any exist, it prints the number of *bytes disk space freed* (clusters × bytes per cluster).
- It then prints, each a right-justified decimal number (32-bit binary to BCD by double-dabble at 46Bh–4C7h, leading zeros suppressed) followed by its text: bytes total disk space (all clusters), disk files (records minus the removed ones), bytes remain available (free clusters × bytes per cluster), bytes total memory (PSP word at offset 2 × 16) and bytes free (that top of memory minus CS, × 16).
- The caller's drive is selected again (AH=0Eh with the byte at 681h) and the program ends with INT 20h.

## Messages

From the data area at 52Ah–6xxh of the loaded image (`$` terminated, for AH=09h):

| Text |
|---|
| `Invalid drive specification` |
| `Invalid parameter` |
| `Allocation error for file ` |
| `Directory error-file: ` |
| `Files cross-linked: ` CR LF, two spaces; and `  and  ` between the two names |
| `File size error for file ` |
| `Diskette not initialized ` |
| ` bytes disk space freed` CR LF CR LF |
| ` disk files` CR LF |
| ` bytes total disk space` CR LF |
| ` bytes remain available` CR LF CR LF |
| ` bytes total memory` CR LF |
| ` bytes free` CR LF |

The exact bytes and their order on screen are fixed by the oracle transcripts, not by this table.

## Observable I/O

See [io.md](io.md).

## Assumption ledger

| ID | Assumption | Why the docs don't settle it | How it is validated |
|---|---|---|---|
| A1 | The statements above come from reading a ~600-line disassembly; addresses, which branch prints which text and the exact repair actions may contain slips | no source listing exists | every statement is replayed against transcripts of the original on the RTL before the Rust code is accepted ([testplan](../plans/testplan.md)) |
| A2 | The original writes the disk only to repair size, allocation and lost-chain errors | the paths at 20E–2E6 and 424h call write functions | disks with injected errors are run through the original and the resulting images compared byte for byte |
| A3 | Only 160 KiB single-sided media (FAT byte FEh) is supported | one media byte compare at 157h | a transcript of the original on a disk with another media byte |

## Observed (oracle, RTL)

| Transcript | Observation |
|---|---|
| `001_workdisk` | the hardware project's formatted data disk reports `64 disk files` and the whole disk available: its directory area is not E5h-filled, so the original counts all 64 slots as files (a quirk of that image, reproduced because the oracle is the original) |
| `002_blank` | a `pcdisk blank` disk (E5h directory) reports `0 disk files`, 160256 bytes total and available |
| `003_dosdisk` | the PC DOS 1.00 disk reports `40 disk files`, 160256 bytes total, 6144 available |
| all | `65536 bytes total memory`, `53392 bytes free` on the 64 KiB machine; the report starts with the file count (no header line); the disks are unchanged afterwards (SHA-256 before = after) |
| all | width: numbers are right-justified in 12 columns, then a space and the text |
