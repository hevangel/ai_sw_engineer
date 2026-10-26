# External projects

Git submodules, all shallow. **Never edit them in place**: patch in our own tree (`llvm-<chip>/target/patches/`, etc.) or fork. Their licenses are their own.

```bash
git submodule update --init --depth 1            # everything
git submodule update --init --depth 1 external/busicom external/chips   # just what you need
```

| Path | Upstream | License | Used for |
|---|---|---|---|
| `llvm-mos/` | [llvm-mos/llvm-mos](https://github.com/llvm-mos/llvm-mos) | Apache-2.0 WITH LLVM-exception | 6502-family LLVM backend (`llvm-mos6502`) |
| `llvm-mos-sdk/` | [llvm-mos/llvm-mos-sdk](https://github.com/llvm-mos/llvm-mos-sdk) | Apache-2.0 WITH LLVM-exception | crt0, linker scripts and libc for C64, NES, Atari 2600, Apple IIe, ... |
| `rust-mos/` | [mrk-its/rust-mos](https://github.com/mrk-its/rust-mos) | MIT / Apache-2.0 | rustc fork targeting llvm-mos (`mos-*-none` targets); also the reference for how to link a rustc to a custom LLVM backend; its `docker/` builds the pinned base image used by our `Dockerfile` |
| `mos6502/` | [mre/mos6502](https://github.com/mre/mos6502) | BSD-3-Clause | Rust 6502 emulator: secondary oracle for 6502 titles |
| `6502_65C02_functional_tests/` | [Klaus2m5/6502_65C02_functional_tests](https://github.com/Klaus2m5/6502_65C02_functional_tests) | GPL-3.0 | CPU conformance programs for 6502 simulator checks (not linked into our code) |
| `martypc/` | [dbalsom/martypc](https://github.com/dbalsom/martypc) | MIT | Cycle-accurate IBM PC/XT emulator in Rust: secondary oracle for 8088 titles |
| `busicom/` | [veniamin-ilmer/busicom](https://github.com/veniamin-ilmer/busicom) | none stated | Rust/WASM Busicom 141-PF emulator (board wiring: keyboard via 4003, printer drum timing, side panel) — secondary oracle and reference for `system/busicom_141pf` |
| `chips/` | [veniamin-ilmer/chips](https://github.com/veniamin-ilmer/chips) | none stated | Rust chip models incl. MCS-4 (`src/mcs4/`: 4004, 4001, 4002, 4003); `busicom` depends on it at `../chips` |
| `rea/` | [morluto/rea](https://github.com/morluto/rea) | MIT | REA, "Reverse Engineer Anything": MCP server and CLI that disassemble, decompile and trace original binaries (Ghidra, IDA or Hopper providers) and record Evidence; the reverse-engineering tool for every original-software rewrite (see `AGENTS.md`) |

`busicom` and `chips` publish no license: read and run them, but do not copy their code into this repository.

## Primary simulators (not a submodule)

The RTL and ISS for 4004, 8008, 6502, 8086/8088 and whole machines (Busicom 141-PF, Micral N, Apple II, IBM PC/XT) live in the sibling repository [ai_hw_engineer](https://github.com/hevangel/ai_hw_engineer), mounted at `$AI_HW_ENGINEER` (default `../ai_hw_engineer`, `/ai_hw_engineer` in Docker).

## Candidates (add when the work starts)

| Upstream | Why not yet |
|---|---|
| [llvm/llvm-project](https://github.com/llvm/llvm-project) | base for the new `llvm-i4004`, `llvm-i8008`, `llvm-i8086` backends; large — add with `--depth 1` when the first backend starts |
| [SingleStepTests/8088](https://github.com/SingleStepTests/8088), [SingleStepTests/65x02](https://github.com/SingleStepTests/65x02) | per-instruction test vectors; several GB |
| [stella-emu/stella](https://github.com/stella-emu/stella), [SourMesen/Mesen2](https://github.com/SourMesen/Mesen2), VICE, AppleWin, MAME | whole-machine oracles for 2600 / NES / C64 / Apple II / everything |
