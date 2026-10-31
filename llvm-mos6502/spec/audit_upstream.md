# Upstream compiler support audit — MOS 6502 family

Audited 2026-10-26 for hevangel/ai_engineer#58. Unlike the i4004/i286 cases,
**upstream support exists**: the 6502 backend is `llvm-mos`, and the
`rust-mos` fork of rustc targets it. The qualification therefore reuses and
pins upstream instead of building a backend.

## Toolchain, pinned and in use

| Component | Version / pin | Source | License |
|---|---|---|---|
| LLVM backend (6502 CodeGen, MC, lld) | clang 20.0.0git / LLD 20.0.0, `--target=mos` | `external/llvm-mos` submodule pin `0f031168a7cc8e81b7b40c0ec0b1f7b3c90b8a63` (shallow, [llvm-mos/llvm-mos](https://github.com/llvm-mos/llvm-mos)); built into the base image | Apache-2.0 WITH LLVM-exception |
| rustc fork | `rustc +mos` = 1.87.0-dev (`1d1d646c0 2025-02-21`), `mos-unknown-none` tier-3 target | `external/rust-mos` pin `c8eda745dc2f5758e1ddbfa035d7c7c6e24fe568` ([mrk-its/rust-mos](https://github.com/mrk-its/rust-mos)); its `docker/` builds the base image | MIT / Apache-2.0 |
| Base image (toolchain + SDK preinstalled) | `mrkits/rust-mos:13f2838f9-334fc98-8f3a80f8` | `Dockerfile` `RUST_MOS_IMAGE` | image of the above |
| SDK (crt0, libc, platform linker scripts) | llvm-mos-sdk `b2b09b72dfca4f8f6fbf1bc3f0ff6c2ccbd9b232` | `external/llvm-mos-sdk` ([llvm-mos/llvm-mos-sdk](https://github.com/llvm-mos/llvm-mos-sdk)); prebuilt at `/usr/local/mos-platform/<platform>` in the image | Apache-2.0 WITH LLVM-exception |
| Host rustc (runners, host-side tools) | 1.99.1 | Dockerfile `RUST_STABLE` | MIT/Apache-2.0 |
| Independent executors | vendored **py65** (`$AI_HW_ENGINEER/design/mos_6502/references/`, BSD/MIT, the ai_hw_engineer 6502 oracle) and **mre/mos6502** (`external/mos6502` pin `ffcc9bcb…`, BSD-3-Clause) | see external/README.md | see table |

## What upstream provides vs what this project owns

- Provided upstream: the 6502 CodeGen/MC/lld, the `mos-unknown-none` rustc
  target (LTO required, panic=abort, zero page as "imaginary registers"
  `__rc0..__rc31`, software stack on page 1), per-platform crt0/libc for C64,
  NES, Atari, ...
- Project-owned for the bare-metal qualification (`llvm-mos6502/`): the reset
  runtime `rt/crt0.s`, the self-contained linker script `spec/link.ld`, the
  smoke program, the expected-value oracle and the two executor comparisons
  ([README.md](../README.md)). Per-machine profiles (KIM-1, Apple II, 2600,
  NES, C64) stay with their machine issues.

## Known skew in the pinned image (recorded, worked around)

The image's `/usr/local/mos-platform/common/lib` INCLUDE-chain linker scripts
use `.zp.data :`-style output sections that the image's own lld 20 refuses to
parse ("unknown directive"), while the self-contained per-platform scripts
(e.g. `c64/lib/link.ld` → `commodore.ld`) link fine. The bare
`mos-unknown-none` target needs a `link.ld` on its `-L` path — this project
supplies its own self-contained `spec/link.ld` and its own `rt/crt0.s`
(`-C link-arg=-nostartfiles`), consuming only `crt0`-independent pieces of
the common platform (the `imag-regs.ld` symbol assignments and `libc.a` for
the compiler-rt arithmetic helpers). If the base image is ever rebuilt from a
newer SDK where the INCLUDE chain parses, `spec/link.ld` can switch back to
`INCLUDE c.ld` and drop the custom crt0; the smoke gates would then re-run.

## Variant policy (6502 / 65C02 / 6507 / 6510 / 2A03)

The backend targets the NMOS 6502 core ISA; variants differ in environment,
not in core codegen: 65C02 adds instructions (`-mcpu=mos65c02` / the
`Mos65C02` emulator variant), 6507 has a 13-bit address bus (Atari 2600
cartridge layout), 6510 adds the DDR/latch IO port (C64 memory map), 2A03
lacks decimal mode (NES). Variant selection is per machine profile — the
base qualification runs the plain NMOS target and documents each machine's
choice; per-variant execution evidence belongs to the machine issues
(Apple II #56/#57, KIM-1 #164/#165).
