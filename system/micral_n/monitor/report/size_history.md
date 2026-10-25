# MIC 01 monitor — size scoreboard

One row per landing that touches the monitor or `llvm-i8008` (AGENTS.md,
"Size scoreboard"). Budget: the 2048-byte EPROM at 0x3800 with entries
pinned at 0x3B30 and 0x3B49.

How each column is measured (ai-sw-engineer image): `scripts/build.sh
target`; **code** is the sum of the `.text` sections in `build/monitor.map`
(or the test map while the ROM did not link), **ROM** the linker's size line
for `layout.ld`; **largest fn** the biggest `.text.*`; host and target from
`cargo test --release` and `scripts/run_sim.sh`.

| Date | Change | Code | ROM | Largest fn | Host | Target |
|---|---|---|---|---|---|---|
| 2026-10-24 | before the fit (4383652) | 3065 | does not fit | `monitor_main` 1672 | 7/7 | 7/7 (test layout) |
| 2026-10-24 | `?` restarts the command loop | 2837 | does not fit | `monitor_main` 1551 | 7/7 | — |
| 2026-10-24 | hold patching in asm; flags as separate bytes | 2508 | does not fit | `monitor_main` 1323 | 7/7 | 7/7 (test layout) |
| 2026-10-24 | best-fit placement (linker); commands split | 2320 | does not fit (272 B over) | `monitor_main` 443 | 7/7 | — |
| 2026-10-24 | checksums in hex8/tape_byte; range statics; page-indexed accesses (backend) | 2221 | 174 B over | `monitor_main` 414 | 7/7 | 7/7 (test layout) |
| 2026-10-24 | compares as values (backend); `G` loop; no register bounds checks; `mon_patch` loop; cold start in the entry gap | 2040 | 2040 / 2048 | `monitor_main` 396 | 7/7 | 7/7 (ROM) |
