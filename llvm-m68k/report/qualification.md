# Qualification record — llvm-m68k (Rust/LLVM target m68k)

Issue: hevangel/ai_engineer#64. Recorded 2026-10-26/27 (America/Vancouver).
All steps run fresh in the `ai-sw-engineer:latest` container (plus the ad-hoc
`binutils-m68k-linux-gnu` install, pinned in the Dockerfile for the next
rebuild).

**Result: NOT qualified.** The gates found two real, reproducible defects —
one in the upstream compiler, one in the candidate executor. Everything below
is evidence; nothing is a claim of a working toolchain.

## What works (verified first)

- The pinned rustc (1.99.1 and nightly-2026-10-15) ships the LLVM M68k
  backend: `core` + `compiler_builtins` compile for `m68k-unknown-none-elf`
  via `-Z build-std=core`; the tier-3 target's default linker
  (`m68k-linux-gnu-ld`) is not in the image and `rust-lld` rejects EM_68K, so
  the qualification links with GNU `m68k-linux-gnu-ld`
  (binutils-m68k-linux-gnu; added to the Dockerfile).
- A probe and a 12-slot mini smoke of hand-verifiable primitives (add, eor,
  and, or, rotate, shifts, 16×16 `mulu`, sub, bit-reverse loop, unsigned
  max-select, popcount via `compiler_builtins`, ror) build to a flat image
  linked at `$400` (entry `_start` verified at `$400`; image SHA-256
  `14613d83bebbefa4d94aa7711456df1a10dcdc7215fea3dda72df607f57417f9` for the
  main smoke).
- **Slots 1–8 + 11–12 of the mini execute byte-identically to the
  Python oracle on r68k** — the backend's straight-line ALU/shift/multiply
  lowering, its `compiler_builtins` popcount call and both rotates are
  correct. GNU objdump (M68000 mode) and the nightly's LLVM objdump agree on
  full `.text` decode coverage.

## Defect 1 — LLVM M68k miscompiles unsigned compare-select (blocks)

At every optimization level ≥ 1 (`-O1`, `-O2`, `-Os`), the lowering of
`if a > b { a } else { b }` (u32) emits:

```
subl  %d3,%d1      ; d1 = a - b, C = borrow (a < b)
movel %d2,%d1      ; d1 = a  — MOVE.L clears C and V on the 68000 (PRM)
bhis  508          ; bhi tests C=0 && Z=0 — now true for every a ≠ 0
```

The flag-clobbering `move.l` is scheduled between the flag-setting subtract
and the flag-using branch: the binary picks `a` for every `a ≠ 0` regardless
of the comparison. Per the M68000 PRM, MOVE clears C/V, so this sequence is
wrong **on real hardware too** — observed identically at `-O1`, `-O2` and
`-Os` (4-byte window divergence, always slot 10), disassembly quoted above.
This is the same pattern that broke the full 32-slot smoke.

Minimal reproducer: `llvm-m68k/rust/smoke` `--bin mini`, slot 10
(`src/mini.rs`); oracle `scripts/mini_expected.py`; sweep script
`build/qrun-opt.sh` (scratch).

Upstream action needed: LLVM M68k must not schedule flag-clobbering
instructions between a flag-setting compare/subtract and its conditional
branch (or must re-test). Until then, `m68k-unknown-none-elf` miscompiles
ordinary Rust comparison code.

**2026-10-28 re-run (own MC68000 executor): CONFIRMED.** The mini was
rebuilt at O0/O1/O2/Os on the pinned `nightly-2026-10-15` with
`binutils-m68k-linux-gnu` and executed on `tools/m68000_run` (the
repository's SingleStepTests-validated MC68000 core) with the windows
relocated to 0x10000/0x11000/0x12000 (the O0 image's `.text` spans
0x400-0x1484 and would otherwise overlap the old 0x1100 window).
Results, byte-exact against `scripts/mini_expected.py`:

| build | own core | r68k (same image) |
| --- | --- | --- |
| mini -O0 | **PASS (48/48 bytes)** | PASS |
| mini -O1 | slot 9 only: got `12345678` want `9abcdef0` | slot 9 only, same bytes |
| mini -O2 | slot 9 only, same bytes | slot 9 only, same bytes |
| mini -Os | slot 9 only, same bytes | slot 9 only, same bytes |

Two independent executors diverge from the oracle in exactly the same
four bytes at every opt level ≥ 1 — the defect is in the compiler, not
in an executor.

## Defect 2 — r68k 0.2.2 misexecutes correct code (RESOLVED — not reproducible)

At `-O0` the emitted select sequence is architecturally correct — the
compare `sub.l` and `bhi` are adjacent (verified in the disassembly; spill
slots confirmed: `$116`=a, `$120`=b, and the *same* `moveml $116,$d0-$d1`
pair-load feeds the passing ADD slot):

```
moveml %sp@(116),%d0-%d1   ; d0 = a, d1 = b
subl   %d1,%d0             ; a - b → C=1 (borrow) for a < b
bhis   1136                ; not taken when C=1
braw   112a
112a: moveml %sp@(120),%d0 ; d0 = b
movel  %d0,%sp@(124)       ; result = b  ← correct semantics
```

A conforming 68000 produces `b` (`0x9ABCDEF0`); the 2026-10-28 re-run
could **not** reproduce this: with correctly-built images (real
`_start` entry, windows clear of `.text`) r68k 0.2.2 and the own
MC68000 core agree with the oracle on the O0 image. The original
divergence is attributed to the scratch harness entering the image at
$400 regardless of where `_start` was placed. **The executor no longer
blocks anything**: r68k and the own core produce identical, oracle-
matching results at O0, and identical slot-9 divergences at O1+ (the
compiler defect).

## Acceptance mapping (issue #64)

| Acceptance box | Evidence |
|---|---|
| Source-backed scope, exact variant and implementation plan | [../spec/audit_upstream.md](../spec/audit_upstream.md) (upstream exists and is pinned; the M68010-default caveat, the linker caveat, licenses) + [../spec/compiler_profile.md](../spec/compiler_profile.md) (M68000 pin, banned 68010+ ops, ABI, harness map) |
| Independent behavior checks and reproducible validation report | This report: oracle (Python source-semantics mirror), two executors/decoders (GNU binutils + r68k + nightly llvm-tools), the opt-level sweep, and the two minimal reproducers — **all reproducible with the commands below** |
| Document remaining limitations and link owning implementation PRs | The two defects above + the limits list; owning PR: hevangel/ai_sw_engineer (this branch) |

## Limits

- Execution oracles: r68k 0.2.2 and the repository's own MC68000 core
  (`tools/m68000_run`, chip/motorola-68000 branch); both now agree on
  every image. The MC68000 RTL remains the long-term primary executor.
- Only the plain-M68000 ISA at `target-cpu=M68000` was exercised; FPU/MMU
  and later-CPU profiles are out of scope.
- The main 32-slot smoke cannot pass until Defect 1 is fixed (slot-select
  pattern); the mini isolates it.

## Reproduce

```sh
# in the ai-sw-engineer container, repo at /workspace
apt-get update -qq && apt-get install -y -qq binutils-m68k-linux-gnu
llvm-m68k/scripts/build.sh                       # compile + link + hash
llvm-m68k/scripts/exec_r68k.sh                   # executes; documents the divergence
llvm-m68k/scripts/disasm_check.py                # decode coverage + banned ops
# minimal reproducer sweep (scratch helper on this branch):
bash build/qrun-opt.sh                           # 4 levels × oracle diff
```
