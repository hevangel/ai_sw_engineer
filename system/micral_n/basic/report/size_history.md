# SCELBAL — size scoreboard

One row per landing that touches the title or `llvm-i8008` (AGENTS.md, "Size
scoreboard"). Budget: the image must end below 0x3700 (14080 bytes), the
boot ROM's load area.

How each column is measured (ai-sw-engineer image): `scripts/build.sh
target`; **.text** and **image** are the linker's size line
(`build/basic.map`, first lines); **largest fn** the biggest `.text.*` in
the map; **calls** the `cal` instructions in `build/basic.s`; host and
target status from `cargo test --release` and `scripts/run_sim.sh`.

| Date | Commit | .text | Image | Largest fn | Calls | Host | Target |
|---|---|---|---|---|---|---|---|
| 2026-10-24 | first build | 15813 | — (21445 with an 18-page buffer: does not fit) | `basic_main` 4396 | — | 10/10 | not run: depth 12 |
| 2026-10-24 | errors restart, page-aligned RAM, one parser call site, RAM image as data | 11080 | 13826 (6-page buffer) | `basic_exec` 3579 | — | 10/10 | 10/10 RAM, 10/10 boot |
| 2026-10-24 | asm carry primitives, array page at 0x3700, 9-page buffer | 10691 | 13826 | `basic_exec` 3579 | 385 | 10/10 | 10/10 RAM, 10/10 boot |
