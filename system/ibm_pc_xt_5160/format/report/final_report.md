# FORMAT (PC DOS 2.00) for the IBM PC XT 5160: report

Issue [hevangel/ai_sw_engineer#288](https://github.com/hevangel/ai_sw_engineer/issues/288). Run of 2026-11-07 on a Linux host.

## Steps of the title lifecycle (spec/software-lifecycle.md)

| Step | Result |
|---|---|
| 1. Original | `FORMAT.COM` from PC DOS 2.00 disk 1, 6,016 bytes, SHA-256 `cd86654e...` ([provenance](../original/provenance.md)); not committed, nor any disk it wrote |
| 2. Original on the emulators | MartyPC `05c0d088` IBM 5160 (256 KiB, XT BIOS 1982-11-08): 12 transcripts with drive B's SHA-256 ([oracle](../oracle/README.md)); our Rust emulator (5150 board, `Machine::new(256)`): screens identical on all 12, drive B identical on 9 and on the other 3 identical but for the volume label's time stamp (below) |
| 3. Source | Microsoft's generic MS-DOS 2.0 FORMAT.ASM, FORMES.ASM, GENFOR.ASM (MIT); IBM's OEM module is not published ([reference](../reference/ms-dos-2.0/README.md)) |
| 4. Documents | the source and the binary are the documents used |
| 5. Reverse engineering | [spec](../spec/spec.md) and [I/O](../spec/io.md) from the binary read in full with GNU objdump (REA needs a Ghidra-class provider, not installed here); the OEM module (0E94h-1241h) from the disassembly alone |
| 6. Rust | `src/lib.rs` (`no_std` core over a `Dos` trait), `src/main.rs` (INT 21h, far accesses, `.COM`), `src/bios.s` (INT 25h/26h, INT 11h, INT 13h) |
| 7-8. Four-way matrix | identical on all 12 cases, screen and drive B, with the masks of the test plan where noted, below |

## The matrix

Screen: the rows from the case's command line to the next `A>`. Drive B: the SHA-256 of the raw image after the run;
"masked" means identical once the label's time word (DOS's clock) and, for /8 without /S, sector 0 (spec D1) are
zeroed ([test plan](../plans/testplan.md)).

| Case | Host replay | Original × Rust emulator | Rewrite × Rust emulator | Rewrite × MartyPC |
|---|---|---|---|---|
| f01 `format b:` | identical | identical | identical | identical |
| f02 used disk | identical | identical | identical | identical |
| f03 `/s` | identical | identical | identical | identical |
| f04 `/v` | masked (label time) | masked (label time) | masked (label time) | identical |
| f05 `/1` | identical | identical | identical | identical |
| f06 `/8` | masked (D1) | identical | masked (D1) | masked (D1) |
| f07 `/1/8` | masked (D1) | identical | masked (D1) | masked (D1) |
| f08 `/s/v` | masked (label time) | masked (label time) | masked (label time) | identical |
| f09 bad label | masked (label time) | masked (label time) | masked (label time) | masked (label time: 0:00:26, the oracle 0:00:24) |
| f10 another | identical | identical | identical | identical |
| f11 `q:` | identical (no drive B) | identical | identical | identical |
| f12 `/x` | identical | identical | identical | identical |

Every screen is identical in every cell. The host replay needs `XT_ROMS` for the drive B images and the /S cases
(without it: the 10 other screens).

### The label time

DOS stamps the volume label with its clock at the create, which counts from the boot; it measures how long the
formatting took on the machine. MartyPC: 0:00:24 (f04, f09), 0:00:28 (f08). Our Rust emulator, original: 0:01:04,
0:01:06, 0:01:16; rewrite there two seconds later (it reads drive A at INIT, spec D6). The rewrite on MartyPC lands on
the oracle's second in f04 and f08 and two seconds later in f09. Emulator finding: formatting 40 tracks x 2 heads
(INT 13h format + verify each) takes about 2.7 times as long on our emulator (ASSUMPTION E-A10's fixed rotation and
step timing) as on MartyPC; nothing else differs, and FORMAT's own output does not depend on it.

### Findings from the oracle images

- The XT BIOS formats 9 sectors a track whatever AL says (the disk base table's count), so /8 disks have a ninth
  F6h sector on every track; both emulators agree.
- FORMAT's FAT buffer (CS:1850h) lies past its file and is cleared only for the FAT's entries: the rest of each FAT
  sector is COMMAND.COM's start-up image (CS:X = COMMAND.COM byte X + 0BC0h), identical on MartyPC and our emulator.
  The rewrite reads those bytes from `A:\COMMAND.COM` (spec D2).
- DOS 2.00's delete of `???????????` marks every entry of the fresh root never used (00h).
- The last partial sector of a system file ends with the bytes after it in FORMAT's buffer (IBMDOS.COM's with
  IBMBIO.COM's start).

## Commands

```bash
IBM_PC_ROMS=DIR XT_ROMS=DIR system/ibm_pc_xt_5160/format/scripts/run_all.sh
```

## Size

`build/format.com`: 8,925 bytes (the original: 6,016), SHA-256 `87b3f78689af8bede8cee1c595ac40c9d212e9fd2fd1d4efe24eec1f18da4ee4`: code 7,946 bytes, read-only data 979; `.bss` 4,165 bytes; the system-file buffer from 3422h to 800h below the top of memory. Built by the msp430 Rust route and `llvm-i8086`.

## Limits and deliberate differences

- D1: the boot record comes from drive A's boot sector (PC DOS 2.00 disk 1's is the original's record but for the OEM name and BPB, which the rewrite sets); for /8 without /S the original writes a DOS 1.10-style record that exists only inside the original FORMAT.COM, which a rewrite run does not have: sector 0 differs (f06, f07, compared masked).
- D2: the FAT buffer's tail from `A:\COMMAND.COM` at 2410h, as the original's memory holds it on PC DOS 2.00 with 256 KiB.
- D3-D7: assembly helper; /B's file contents; size; buffer placement and drive A reads at INIT; the fixed-disk path ported but not run.
- Not covered by a case: bad sectors and tracks, write protection, /B, fixed disks, ASSIGN, a missing system disk, Ctrl-Break. Not run on hardware.
