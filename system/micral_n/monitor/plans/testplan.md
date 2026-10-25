# MIC 01 monitor test plan

| Level | What | Oracle | Command | Image |
|---|---|---|---|---|
| Host replay | `oracle/transcripts/*.txt` except `t*`, through a mock machine | the manual (spec) | `cargo test --release` | ai-sw-engineer |
| Target | `build/monitor_test.bin` on the RTL, transcripts except `h*` | same | `scripts/run_sim.sh` | ai-hw-engineer |
| ROM fit | `layout.ld` links within 2 KiB | — | `STRICT=1 scripts/build.sh target` | ai-sw-engineer (fails today) |

Transcript prefixes: `h` host only (the mock's `G` returns to the command
loop), `t` target only (a real hold). `=` lines preload RAM, `R` lines the
saved registers, `?` lines check memory after the run.

| Transcript | Covers |
|---|---|
| 01_m_display_modify | M: display, modify, keep, end |
| 02_errors | unknown letters, a fifth address digit, `?` mid-command |
| 03_dump | D, partial first line, `..` |
| 04_registers | A, H, I/T with flags |
| h05_go_holds | G with holds (mock) |
| 06_tapes | C, P, F, N |
| 07_read_tape | R, checksum error |
| t08_hold | G into a program that reaches a hold, on the RTL |
