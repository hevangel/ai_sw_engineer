# graphics (ibm_pc_xt_5160)

| | |
|---|---|
| Original | `GRAPHICS.COM` of IBM PC DOS 2.00 (789 bytes, directory date 1983-03-08): "The IBM Personal Computer Graphics Printer Support Version 1.00" |
| Original year | 1982-1983 (the binary's text: "(C)Copyright IBM Corp 1982"; shipped with PC DOS 2.00, whose directory date is 1983-03-08) |
| Original authors | M. Hallerman for IBM (the binary's text: "Author - M. Hallerman"); no source is known to be published |
| Machine | ibm_pc_xt_5160 — i8086 |
| Issue | [hevangel/ai_sw_engineer#299](https://github.com/hevangel/ai_sw_engineer/issues/299) |
| Status | reimplemented (Rust transient part, assembly resident handler); the four-way matrix (original/rewrite × MartyPC 5160/Rust emulator) and the host replay are identical on all 11 cases, screen and printed bytes ([report](report/final_report.md)) |

## What it does

`graphics` installs an INT 5 (Print Screen) handler and stays resident in 296h bytes (INT 27h), its code moved down
over its own PSP. In the CGA's graphics modes the handler reads the screen pixel by pixel (INT 10h 0Dh) and prints
it on an IBM Graphics Printer through INT 17h with bit-image escapes (`ESC L`, 24/216-inch lines): 320 x 200 in
four colours as three dot columns by two pins a pixel, 640 x 200 turned a quarter turn. Text modes go to the
handler it found (the BIOS Print Screen). It shows nothing and does not check for an installed copy. Details in
[spec](spec/spec.md).

No emulator here hands over the bytes of a graphics print, so the cases capture INT 17h inside the guest with this
repository's own tools ([tools/](tools/README.md): PRNCAP.COM keeps the bytes, DRAW.COM draws a picture and calls
INT 5, PRNDUMP.COM writes them to drive B).

## Layout

| Path | Contents |
|---|---|
| [spec/](spec/spec.md) | behaviour from the binary (and the BIOS Print Screen it chains to), what the cases printed, [I/O contract and resident layout](spec/io.md), deliberate differences D1-D3, assumptions A1 and H1-H4 |
| [original/](original/provenance.md) | provenance and SHA-256 of the original (binary gitignored) |
| [oracle/](oracle/README.md) | transcripts of the original on MartyPC's IBM 5160: screen rows, drive B's hash, the printed bytes' length and SHA-256 |
| [tools/](tools/README.md) | the capture tools (assembly) and their build script |
| `src/` | `lib.rs` the `no_std` core: the transient part (`install`) over a `Dos` trait and the handler's logic (`int5`, for the host); `resident.s` the resident handler at the original's offsets and the start at 335h (spec D1, D2); `main.rs` the target's `Dos` |
| `tests/` | `oracle_replay.rs` (host: the core and the handler model in front of a model of the machine, DRAW, PRNCAP and the BIOS; the spec's tests), `rust_emulator.rs` (original and rewrite on the Rust emulator; the build's layout), `tools.rs` (the tools on the Rust emulator), `common/` (cases, disks, the comparison rule, DRAW's picture) |
| `scripts/` | `cases.py`, `capture.sh` (capture_dos.sh with SAVE_B=1 and the printed bytes' hash), `build.sh`, `compare.py`, `decode.py` (printer bytes to lines and a PBM), `run_all.sh` |
| [plans/testplan.md](plans/testplan.md) | the cases, the comparison rule, the levels |
| [report/](report/final_report.md) | results |

## Run

```bash
IBM_PC_ROMS=DIR XT_ROMS=DIR MARTY_TARGET_DIR=DIR CARGO_TARGET_DIR_LD=DIR system/ibm_pc_xt_5160/graphics/scripts/run_all.sh
```

`cargo test` alone runs the host replay of the 11 cases (without `XT_ROMS` the graphics cases' printed bytes are
not compared: they need IBM's ROM font) and the tests from the spec. The rewrite on MartyPC alone:

```bash
XT_ROMS=DIR MARTY_TARGET_DIR=DIR PROGRAM=$PWD/build/graphics.com NAME=GRAPHICS.COM \
  OUT=$PWD/build/rust_on_martypc WORK=$PWD/build/rust_run scripts/capture.sh
python3 -I scripts/compare.py oracle/transcripts build/rust_on_martypc
```

Never run `capture.sh` without `PROGRAM` unless the oracle is to be re-captured.
