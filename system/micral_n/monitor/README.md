# MIC 01 monitor (micral_n)

| | |
|---|---|
| Original year | 1974 (documented in R2E's *MICRAL N User's Manual*, January 1974) |
| Original authors | R2E |
| Machine | Micral N — i8008 |
| Status | behavioral rewrite in its 2 KiB EPROM: ROM image 2040 / 2048 B; host 7/7, RTL 7/7 from the ROM at 0x3800 (6 shared transcripts, 1 host-only, 1 target-only) |

## History

MIC 01 is the Micral N's 2 KiB EPROM monitor: memory and register display
and modify, dump, run with holds (breakpoints), paper tape punch and read
on the teletype. The manual describes it in chapter II; MO5 dumped the ROM
but did not publish it, so the manual is the oracle (spec/spec.md).

## Layout

| Path | Contents |
|---|---|
| [spec/](spec/spec.md) | commands from the manual, assumption ledger MON-1..11 |
| `src/` | `lib.rs` commands, `hal.rs` machine trait, `main.rs` target and host entries |
| `asm/entry.s` | cold and warm entries, the `?` restart, hold entries and hold patching, `G` trampoline |
| `layout.ld` | the ROM at 0x3800, entries pinned at 0x3B30 / 0x3B49 |
| [oracle/](oracle/README.md) | transcripts written from the manual |
| `tests/` | host replay |
| [plans/](plans/testplan.md) | test plan |
| `scripts/` | build, `run_target.py` RTL replay, run_all |
| [report/](report/final_report.md) | final report and [size history](report/size_history.md) |

## Run

```bash
./scripts/run_all.sh     # lint, host replay, ROM image (ai-sw-engineer); RTL replay (ai-hw-engineer)
```
