# Micral N boot ROM (micral_n)

| | |
|---|---|
| Original year | 1973, with the machine (the floppy boot EPROM on the 4 KiB RAM card; spec "Sources") |
| Original authors | R2E |
| Machine | Micral N — i8008 |
| Status | behavioral rewrite: 241 B of the 256-byte EPROM; boots Shooting Stars (4/4) and SCELBAL (10/10) from disk images on the RTL; host disk-model tests 2/2 |

## History

The Micral N's 256-byte bootstrap EPROM reads the first 14 KiB of a floppy
into RAM at 0 and starts it. MO5 dumped the EPROM but did not publish its
bytes, so this is a rewrite from MO5's and HxC's descriptions and the MO5
emulator's device model (spec/spec.md).

## Layout

| Path | Contents |
|---|---|
| [spec/](spec/spec.md) | behavior and assumption ledger BOOT-1..4 |
| `src/` | `lib.rs` seek, sector loop, checksum, retry; `hal.rs` disk and stack-card trait; `main.rs` target entry |
| `asm/io.s` | `start` and the disk/stack-card drivers as `__leaf_` routines |
| `layout.ld` | the EPROM at 0x0000, frames at 0x3700 (BOOT-3) |
| `tests/disk_model.rs` | a disk and stack-card model on the host |
| [plans/](plans/testplan.md) | test plan |
| `scripts/` | build, RTL replay (boots the other titles), run_all |
| [report/](report/final_report.md) | final report |

## Run

```bash
./scripts/run_all.sh     # lint, host tests, image (ai-sw-engineer); RTL boot of the titles (ai-hw-engineer)
./scripts/run_sim.sh     # after building shooting_stars and basic
```
