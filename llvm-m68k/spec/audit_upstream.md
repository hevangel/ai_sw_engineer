# Upstream compiler support audit — Motorola 68000 family (m68k)

Audited 2026-10-27 for hevangel/ai_engineer#64. Unlike the 6800/6809 audits,
**a working Rust/LLVM compiler exists today and is already in this repo's
pinned toolchain** — m68k is the reuse case, like llvm-mos for the 6502.

## LLVM / Rust (verified working in the pinned image, 2026-10-27)

- **LLVM M68k backend**: upstream since LLVM 13 (experimental); the pinned
  rustc binaries ship it. Verified in the `ai-sw-engineer` container:
  both `1.99.1` (Dockerfile `RUST_STABLE`) and `nightly-2026-10-15`
  (`RUST_NIGHTLY`) list `m68k-unknown-linux-gnu` and
  `m68k-unknown-none-elf`; `core` + `compiler_builtins` **compile
  successfully** for `m68k-unknown-none-elf` via `-Z build-std=core`, and a
  linked probe produces correct M68000 code (a constant-folded
  `move.l #57266238,d0` — hand-verified against the source arithmetic).
- **Known miscompile (found by this qualification, 2026-10-27)**: at any
  optimization level ≥ 1, unsigned compare-select lowers to a
  flag-clobbering sequence (`sub.l` → `move.l` → `bhi`; MOVE clears C/V on
  the 68000), picking the wrong operand whenever the tested value is
  non-zero. Minimal reproducer + disassembly:
  [../report/qualification.md](../report/qualification.md) Defect 1. This
  blocks general qualification until fixed upstream (LLVM M68k
  flag-restore/re-materialization between compare and branch).
- Target spec (from `--print target-spec-json`): `arch m68k`,
  `cpu M68010` (default — pin `-C target-cpu=M68000` for plain-68000
  output), big-endian data layout
  `E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16`.
- **Caveat (project-owned workaround)**: the tier-3 target's default linker
  is `m68k-linux-gnu-ld`, which is not in the image; `rust-lld` rejects
  m68k objects (`unsupported e_machine value: 4`). The qualification links
  with the GNU cross linker `m68k-linux-gnu-ld` (binutils-m68k-linux-gnu,
  GPLv3 — a build/link tool, not copied into artifacts) and pins that
  requirement in the Dockerfile. A custom target JSON with
  `-C linker-flavor` self-contained lld can revisit this when lld gains
  EM_68K linking.

## Binutils (linker + independent decoder)

- `binutils-m68k-linux-gnu` (GNU, GPLv3): `m68k-linux-gnu-ld` for linking
  (`-e _start -Ttext=… --oformat binary`) and `m68k-linux-gnu-objdump` as
  the **independent disassembler** (GNU, not LLVM) for opcode verification,
  run in M68000 mode.

## Independent executor

- **r68k 0.2.2** ([crates.io](https://crates.io/crates/r68k),
  [github.com/slippyex/r68k](https://github.com/slippyex/r68k)) — MIT,
  zero dependencies, a Rust port of Karl Stenerud's
  [Musashi](https://github.com/kstenerud/Musashi) 680x0 core, cycle-accurate
  and property-tested against Musashi. Used as the execution oracle for this
  qualification (pinned `=0.2.2` in [../tools/r68k_run/Cargo.toml](../tools/r68k_run/Cargo.toml)).
  **Known defect found by the qualification**: misexecutes the
  architecturally-correct −O0 compare/select sequence (see
  [../report/qualification.md](../report/qualification.md) Defect 2).
- MAME's M68k family cores and WinUAE's are heavier alternatives.
- **ai_hw_engineer MC68000 RTL** (`design/motorola_68000`) is a
  work-in-progress skeleton: SingleStepTests 680x0 harness (1,000,060
  vectors) is ready, CPU increments are landing. It is the future primary
  executor; this qualification records it as a limit.

## Consequence for the profile (issue #64)

Reuse upstream everything, and own the glue: the bare-metal link setup
(linker script/flags), the smoke qualification suite, the Python oracle and
the r68k execution gate ([README.md](../README.md)). No backend work is
needed; no compiler claims beyond upstream's are made.
