# Qualification record — llvm-mos6502 (Rust/LLVM target mos6502)

Issue: hevangel/ai_engineer#58. Recorded 2026-10-26 (America/Vancouver). All
steps run fresh in the `ai-sw-engineer:latest` container (base image
`mrkits/rust-mos:13f2838f9-334fc98-8f3a80f8`, image ID `15c2601a16ff`).

## Toolchain under qualification

- `rustc +mos` 1.87.0-dev (rust-mos, `external/rust-mos` pin `c8eda745…`),
  target `mos-unknown-none` (tier 3, LTO required, panic=abort),
  `-Z build-std=core` (compiler-builtins from the mos-patched fork).
- llvm-mos clang/lld 20.0.0git (`--target=mos`), `external/llvm-mos` pin
  `0f031168…`; SDK `external/llvm-mos-sdk` pin `b2b09b72…`.
- Host rustc 1.99.1 for the mre/mos6502 runner.

## Gates

1. **Build** — `llvm-mos6502/scripts/build.sh`: `llvm-mc -arch=mos` assembles
   `rt/crt0.s`; `cargo +mos build --release --target mos-unknown-none -Z
   build-std=core` links the smoke program with our `spec/link.ld` and crt0
   (`-nostartfiles`); `llvm-objcopy -O binary` produces the flat 32 KiB image
   ($8000..$FFFF).
   - **SHA-256 `1b471700c7aafef3fb981fd50cf33bc6c123ffe3ea98737297fb686834aa0814`**,
     bit-identical across two consecutive builds.
   - `.text` = 0x17a bytes (main 0x8000.., crt0 `_start` at 0x80a5 area,
     park/trap self-loops); RESULTS window at 0x0200 (32 B), DONE sentinel at
     0x0220; vectors: NMI/IRQ → `__trap` (0x8177), RESET → `_start` (0x80a5).
   - Build-script caveat fixed during qualification: cargo does not track
     `-C link-arg` object files and its "relink" re-hardlinks the cached
     deps artifact — the crt0 object is therefore passed under a
     content-hashed name so every crt0 change invalidates cargo's fingerprint.
2. **Independent opcode verification** — `scripts/disasm_check.py` with the
   vendored py65 opcode table (not llvm-mos's own objdump): the RESET vector
   equals the ELF `_start`; all **378 .text bytes walk as documented NMOS
   opcodes** (no `???` entries, stream stays in sync via per-mode lengths);
   the disassembly covers every instruction kind the smoke source requires
   (LDA/STA/ADC/SBC/EOR/LSR/ROR/CMP/INX/JMP/JSR/TXS/RTS); park and `__trap`
   decode as JMP-to-self.
3. **Execution, independent emulator #1** — `scripts/exec_py65.py` on the
   vendored py65 (ai_hw_engineer's 6502 oracle): runs from the RESET vector,
   **PASS — the 32-byte results window is byte-identical to the independent
   expected values** (2,510 instructions; final A=0x31 X=0x5a Y=0x80 SP=0xfd).
4. **Execution, independent emulator #2** — `scripts/exec_mos6502.sh` on
   external/mos6502 (mre, BSD-3) via `tools/mos6502_run`: **PASS — identical
   window**, and the same **2,510-instruction step count and final
   registers** as py65. Expected values come from `scripts/expected.py`
   (the smoke's arithmetic re-implemented in Python) — never from the
   compiled image or either emulator.

## Acceptance mapping (issue #58)

| Acceptance box | Evidence |
|---|---|
| Survey upstream support with versions/sources | [../spec/audit_upstream.md](../spec/audit_upstream.md): llvm-mos/rust-mos pins, licenses, base-image pin, SDK |
| Specify ISA subset, ABI, data layout, runtime, linker/memory budget | NMOS documented set (verified by gate 2); llvm-mos ABI (zero page = imaginary registers `__rc0..__rc31`, page-1 software stack, LTO); memory map fixed by `spec/link.ld` with the runtime contract in `rt/crt0.s`; no fixed per-machine budget here — machine profiles (KIM-1, Apple II, …) own their budgets |
| Compile representative no_std Rust; verify emitted opcodes independently | Gates 1–2 |
| Execute target images on an independent machine oracle; report fit/limits | Gates 3–4 (two independent emulators, oracle-derived expected values) |

## Limits and explicit gaps

- The image skew workaround: the pinned image's `common` platform INCLUDE
  chain does not parse with its own lld (details in the audit), so the bare
  qualification uses a project-owned crt0 + link script instead of the SDK's
  machine-agnostic crt0. Consequence: init/fini-array walking is not
  performed by this crt0 (no C++-style constructors are used by core-only
  Rust); a future SDK/toolchain refresh can restore the SDK path and re-run
  the gates.
- Decimal mode: NMOS 6502 has BCD ADC/SBC; the smoke program and llvm-mos
  codegen use binary mode (crt0 issues `cld`). Decimal behavior is the CPU's
  (already covered by ai_hw_engineer's RTL gates incl. Klaus Dormann), not
  the compiler's; a decimal-mode compiler test is not claimed.
- One representative program: this qualification proves the toolchain on the
  smoke's patterns, not every LLVM lowering path. The per-machine profiles
  will compile real consumer software (BASIC, monitor, games) as the wider
  regression.
- Executors are user-level 6502 emulators, not machine RTL. The primary
  cross-check — running the compiled image on ai_hw_engineer's 6502 RTL
  (Klaus-verified) — is future work shared with the KIM-1/Apple II machine
  profiles (their harnesses are fixed-test benches today).
- 6507/6510/2A03/65C02 variants: policy documented in the audit; per-variant
  evidence belongs to the machine issues.
