# Micral N memory map

The 8008 addresses 16 KiB in 64 pages of 256 bytes ([R2E] ch. III). Cards are
strapped to addresses; this map is the machine MO5 restored ([HxC] card
table), which our simulators reproduce.

| Start | End | Size | Kind | Use | Source |
|---|---|---|---|---|---|
| 0x0000 | 0x00FF | 256 | EPROM over RAM | boot ROM (read until the first `RST 0`; writes go to RAM) | [HxC] card 6, [MO5emu] MemoryCard |
| 0x0000 | 0x37FF | 14 KiB | RAM | programs loaded by the boot ROM or the monitor | [HxC] cards 1–6 |
| 0x07E0 | 0x07FF | 32 | RAM | MIC 01 monitor variables (in the original) | [R2E] §II.2.2 |
| 0x3800 | 0x3FFF | 2 KiB | EPROM | MIC 01 monitor; entry 0x3B30 at power-on, 0x3B49 after | [R2E] §II.2.2, §II.3.1; [HxC] card 21 |

Sources as in `io_map.md`.

## Per-image budgets

| Image | Code | RAM for data, frames | Notes |
|---|---|---|---|
| boot ROM | 0x0000–0x00FF (256 B) | 0x3700–0x37FF | runs before anything is loaded; loads 14 KiB to 0x0000 |
| monitor | 0x3800–0x3FFF (2 KiB) | 0x0700–0x07FF | see the monitor's spec for its RAM deviation |
| programs (BASIC, Shooting Stars) | from 0x0000 (boot) or 0x0100 (monitor `R`/`G`) | after the code, up to 0x36FF | 0x3700–0x37FF stays free for the boot ROM's buffer |

The 8008 stack has 7 return levels. A program that the monitor starts with
`G` returns into the monitor only through its breakpoints, so it may use all 7.
