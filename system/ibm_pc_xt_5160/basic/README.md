# basic (ibm_pc_xt_5160)

| | |
|---|---|
| Original | IBM PC **Disk BASIC D2.00** (`BASIC.COM` of PC DOS 2.00, 16,256 bytes, dated 1983-03-08) running on **Cassette BASIC C1.10** in the XT's ROM (F600:0000, 32 KiB) |
| Original year | ROM 1981 (`25-Apr-81` in the ROM), BASIC.COM 1983 (`17-Jan-83` in the file; PC DOS 2.00) |
| Original authors | Microsoft for IBM (the same code base as Microsoft's GW-BASIC, [reference](reference/README.md)) |
| Machine | ibm_pc_xt_5160 — i8086 |
| Issue | [hevangel/ai_sw_engineer#301](https://github.com/hevangel/ai_sw_engineer/issues/301) |
| Status | **M1 done** (2026-11-10): a medium-model Rust BASIC.COM (start, banner, `Bytes free`, key line, screen editor, tokenizer and lister, program lines, direct-mode PRINT/KEY/WIDTH/CLS/ERROR/LIST/NEW/RUN/SYSTEM, every error message); 9 of 31 cases identical in all three cells it runs in (host model, Rust emulator, MartyPC); the original identical on the Rust emulator on all 31. Plan: [plans/implementation_plan.md](plans/implementation_plan.md) |

## What it is

Microsoft's BASIC interpreter as IBM shipped it: the ROM holds the interpreter, BASIC.COM adds disk files,
DOS 2.0 paths and the extended statements and patches itself into the ROM. The rewrite is one Rust
`BASIC.COM` that needs no ROM BASIC ([spec](spec/spec.md)).

## Layout

| Path | Contents |
|---|---|
| [spec/spec.md](spec/spec.md) | behaviour: layout, tokens, lines, variables, MBF numbers, printing, errors, editor, files, SAVE formats, ledger |
| [spec/io.md](spec/io.md) | memory, BIOS and DOS calls, file formats |
| [plans/implementation_plan.md](plans/implementation_plan.md) | toolchain findings with measurements, architecture, milestones, risks, open decisions |
| [plans/medium_model/](plans/medium_model/README.md) | the medium-model proof (code in several segments) |
| [plans/size_probe/](plans/size_probe/README.md) | the code-size probe |
| [original/provenance.md](original/provenance.md) | the two originals (gitignored) |
| [reference/README.md](reference/README.md) | GW-BASIC source (MIT), linked by commit |
| [oracle/](oracle/README.md) | transcripts of the originals on MartyPC's IBM 5160 |
| `src/` | `lib.rs` the `no_std` core over `hal::Hal` (`interp`, `screen`, `token`, `tables`, `program`, `errors`); `main.rs` the target (BIOS/DOS through llvm-i8086's `rt/medium/far.s`, the data segment and video memory by inline far access) |
| `tests/` | `oracle_replay.rs` (host model), `rust_emulator.rs` (original and rewrite on the Rust emulator), `common/` |
| [plans/testplan.md](plans/testplan.md) | levels, the comparison rule, cases per milestone |
| `scripts/cases.py` | the 31 cases |
| `scripts/build.sh` | `build/basic.com`, the medium-model `.EXE` (llvm-i8086/spec/abi.md D10) |
| `scripts/compare.py` | a run's transcripts against the oracle's |

## Run the oracle

```bash
XT_ROMS=<dir from toolchain/scripts/fetch_reference.py> \
  bash system/ibm_pc_xt_5160/scripts/capture_dos.sh $PWD/system/ibm_pc_xt_5160/basic b10_numfmt ...
```

## Build and test

```bash
bash system/ibm_pc_xt_5160/basic/scripts/build.sh                     # build/basic.com
cargo test --release                                                   # host model (in basic/)
IBM_PC_ROMS=<5150 ROMs> XT_ROMS=<xt_ref> cargo test --release --test rust_emulator -- --ignored
PROGRAM=$PWD/system/ibm_pc_xt_5160/basic/build/basic.com NAME=BASIC.COM OUT=/tmp/run XT_ROMS=<xt_ref> \
  bash system/ibm_pc_xt_5160/scripts/capture_dos.sh $PWD/system/ibm_pc_xt_5160/basic b14_errmsg1 ...
python3 system/ibm_pc_xt_5160/basic/scripts/compare.py system/ibm_pc_xt_5160/basic/oracle/transcripts /tmp/run
```
