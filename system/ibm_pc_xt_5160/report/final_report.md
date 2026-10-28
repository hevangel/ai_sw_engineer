# Report: XT 5160 toolchain profile (ai_sw_engineer#59) — INTERIM

Date: 2026-10-28. Status: profile drafted and self-consistent; no Rust code
has been compiled or executed for this target. This is not a working
compiler claim.

## Acceptance-box status

| Box | State | Evidence |
|---|---|---|
| 1 audit | done | `spec/toolchain_audit.md` |
| 2 machine/CPU spec | done (ABI/layout provisional) | `spec/cpu_abi.md` |
| 3 target spec | provisional draft | `rust/i8086-xt-none.json` |
| 4 startup/linker/runtime | design only | `platform/link.ld`, `platform/runtime/README.md` |
| 5 build/package commands | chosen, unexecuted | `cpu_abi.md` §8, `bringup/build.sh` (needs NASM) |
| 6 smoke compile + decode | blocked | needs ai_engineer#60 backend |
| 7 execute on qualified oracle | blocked | needs backend; oracles identified, baseline unrun |
| 8 tests + report | partial | `tests/test_profile.py` 22/22; this file |

## Commands run (this session)

- `bash scripts/new_system.sh ibm_pc_xt_5160 i8086` → scaffolded.
- `rustc --version` → 1.99.1; `rustc --print target-list` → 331 targets,
  smallest x86 i586 (no 16-bit target).
- `python3 system/ibm_pc_xt_5160/tests/test_profile.py` → 22/22 PASS
  (WSL Python 3.12.3).
- Upstream sources fetched: LLVM `x86-16.ll` (proves code16 = i386, not
  8086), MartyPC README (5150/5160, hw-validated 8088), Open Watcom v2
  README, gcc-ia16/build-ia16 indexes, NASM/Capstone/ia16-model docs.

## Versions pinned

Dockerfile: rustc 1.99.1 / nightly-2026-10-15 / lit 18.1.8 /
`mrkits/rust-mos:13f2838f9-334fc98-8f3a80f8`; LLVM base branch
`llvmorg-23.1.3` (submodule not initialized). MartyPC: release 0.4.1 /
dev 0.5.0 (submodule not initialized; commit pin at bring-up). NASM,
gcc-ia16, Open Watcom, Capstone: pin at implementation.

## Explicit gaps (not claims)

1. `llvm-i8086` does not exist; all (P) spec parts await ai_engineer#60.
2. `bringup/build.sh` never executed (no NASM in session); no sha256 yet.
3. No oracle has booted anything from this folder; L2–L4 unrun.
4. Linked issues (#29, ai_engineer#60/#83/#84) not re-read this session
   (`gh` auth unavailable); #59 fields (Path/Start date/Status/Agent) not
   updated — first follow-up with auth.
5. Licenses of gcc-ia16/Open Watcom/NASM recorded from memory of upstream,
   flagged verify-at-pin in the audit.

## Limits / unsupported

Near-only profile (≤64 KiB code, ≤64 KiB data+stack); no far pointers, no
FP codegen, no `i64`/`i128`, no MZ `.EXE` yet, no interrupts in crt0.
See `spec/cpu_abi.md` §10 (XT-A1..A4).
