# FORMAT test plan

The oracle is the original `FORMAT.COM` of PC DOS 2.00 run by PC DOS 2.00 on MartyPC's IBM 5160 (256 KiB) with the
original XT BIOS (`../scripts/capture_dos.sh` with `SAVE_B=1` → `oracle/transcripts/`). Each case
(`scripts/cases.py`) is a command typed at `A>`, a drive B disk, and (wait text, keys) pairs for what is typed
after it. The drive B disk is an image cases.py writes (`b:image:ZERO360.IMG`, `b:image:ZERO180.IMG`: zero-filled
360 KiB and 180 KiB images) or a blank 360 KiB disk built with pcdisk's steps (`b:mkdir:`, `b:add:`); `-` leaves
drive B empty. Every level builds that disk the same way (`tests/common/mod.rs`). `strike any key` is answered with
a space and the Y/N question with one key (FORMAT reads single keys); the label is typed with Enter. A wait text
starting with `$` means the last non-blank row ends with the rest, once the screen has changed since the previous
match (`until endswith`).

## What is compared

Two things, both of which must agree:

- **The screen**, empty rows included, right-trimmed: if the oracle's screen shows the case's command line (`A>` +
  the command), the rows from the one after it to the next `A>`, in the oracle and in the run (from the run's last
  such command line); otherwise the oracle's rows from the top through its final `A>` against the same number of
  rows of the run ending with its final `A>`. Every FORMAT case shows its command line.
- **Drive B** after the run: the SHA-256 of the raw image (`# drive B after: sha256 <hex>` in the transcript; f11 has
  no drive B). Where it differs, the image with two masks is compared with `oracle/drive_b_masked.txt` (the
  oracle's images with the same masks), and the cell is reported as "masked":
  - the time word (bytes 22-23) of a volume label entry in the first root sector: DOS stamps the label with the
    machine's clock, which depends on how long the formatting took on that machine (f04, f08, f09);
  - for `/8` without `/S`, sector 0: the original writes a DOS 1.10-style boot record from its own image there,
    which the rewrite cannot have (spec D1; f06, f07).

The rule is in `tests/common/mod.rs` (`compared`, `masked`, `drive_b_result`) and `scripts/compare.py` (the command
from the transcript's `# oracle:` header; the run's images from capture_dos.sh's WORK directory).

| Case | Disk B / command / keys | What it checks (spec section) |
|---|---|---|
| f01 | zero 360 KiB, `format b:`, space, `n` | the 360 KiB layout: 40 tracks x 2 heads formatted (F6h), the boot record with the BPB, both FATs with COMMAND.COM's bytes after the entries, the root's entries marked never used by the delete, the report (SYSLOOP, CLEAR, REPORT) |
| f02 | 360 KiB with `ROOT.TXT` and `SUB\A.TXT`, `format b:` | a used disk comes out the same as a zero one (f01's hash) |
| f03 | zero 360 KiB, `format b:/s` | IBMBIO.COM, IBMDOS.COM, COMMAND.COM copied in that order with the sources' dates, attributes 27h/27h/20h, the last sectors' tails; `System transferred`, `40960 bytes used by system` (READDOS, FATWRT, GETSIZE) |
| f04 | zero 360 KiB, `format b:/v`, `MYDISK` | the label prompt and entry (VOLID) |
| f05 | zero 360 KiB, `format b:/1` | the 180 KiB layout on head 0 only; head 1 untouched (DISKFORMAT) |
| f06 | zero 360 KiB, `format b:/8` | the 320 KiB layout with a ninth F6h sector on every track; DONE's directory (attribute 06h in entries 0-1, E5h); sector 0 masked (spec D1) |
| f07 | zero 180 KiB, `format b:/1/8` | the 160 KiB layout on a single-sided disk; DONE; sector 0 masked |
| f08 | zero 360 KiB, `format b:/s/v`, `SYS DISK` | /S and /V: the label after the three files |
| f09 | zero 360 KiB, `format b:/v`, `BAD.NAME`, then `GOOD` | `Invalid characters in volume label` and the prompt again |
| f10 | zero 360 KiB, `format b:`, `y`, space, `n` | `Format another`: CR LF CR LF and a second disk (the same disk again: f01's hash) |
| f11 | `format q:` | the invalid drive check on AL at entry |
| f12 | zero 360 KiB, `format b:/x` | `Invalid parameter`; drive B untouched (the zero image's hash) |

| Level | Runs | Expected | Command |
|---|---|---|---|
| Host | the Rust core with a DOS 2.00 and BIOS model over the disk images (spec/io.md; `tests/oracle_replay.rs` describes the model) | the oracle's rows; drive B identical or masked (needs `XT_ROMS`; without it the screens of the cases without /S) | `cargo test` |
| Original × Rust emulator | the original under PC DOS 2.00 on `system/ibm_pc/emulator` (5150 board, `Machine::new(256)`, two drives; drive B read back from `board.fdc.disks[1]`) | the oracle's rows and drive B | `cargo test --release --test rust_emulator -- --ignored` |
| Rewrite × Rust emulator | `build/format.com` in place of FORMAT.COM there | the same | same |
| Rewrite × MartyPC 5160 | `build/format.com` in place of FORMAT.COM on the same disk | the same | `capture_dos.sh` with `SAVE_B=1 PROGRAM=`, then `compare.py` with the WORK directory |
| Original × MartyPC 5160 | the oracle itself | — | `capture_dos.sh` with `SAVE_B=1` |

Not covered by a case: bad sectors and bad tracks (BADSECTOR's error return, BAD100), track 0 bad, a second side that
does not verify (the single-sided conversion), write-protected or missing disks (INT 13h 03h/80h), /B, /8/V
(`Parameters not compatible`), fixed disks, ASSIGN, a COMMAND.COM that does not fit the memory, the system files
missing from drive A (`Insert DOS disk`), a switch character other than `/`, Ctrl-Break, DOS other than 2.00.
