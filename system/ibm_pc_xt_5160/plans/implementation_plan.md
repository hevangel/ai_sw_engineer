# Implementation plan: XT 5160 toolchain profile (ai_sw_engineer#59)

Owner of the per-computer Rust profile; the shared `llvm-i8086` backend is
ai_engineer#60's. Work order follows the issue's acceptance boxes. Status as
of 2026-10-28.

## Done (this change)

1. Scaffolded `system/ibm_pc_xt_5160/` (`new_system.sh ibm_pc_xt_5160 i8086`).
2. Box 1 audit: `spec/toolchain_audit.md` (upstream gap proven, tools+oracles pinned).
3. Box 2 machine/CPU spec: `spec/cpu_abi.md` (8088-only ISA, near-only ABI (P), layout (P), ledger XT-A1..A4).
4. Box 3 target draft: `rust/i8086-xt-none.json` (provisional, layout-verbatim).
5. Box 4 design: `platform/{memory_map,io_map,link.ld}` + `platform/runtime/README.md` contract.
6. Box 5 packaging choice: boot sector first, `.COM` second, MZ later (`cpu_abi.md` §8).
7. Box 8 (partial): `tests/test_profile.py` (22/22 green) + `report/final_report.md` (INTERIM).
8. Oracle baseline source: `bringup/boot_hello.asm` + `build.sh` + `expected.txt` (unassembled).

## Next, in order (backend-independent)

1. Add NASM to the Docker image, pin version, run `bringup/build.sh`,
   record sha256 in `bringup/expected.txt`. (Blocks all execution evidence.)
2. Initialize `external/martypc` at a pinned commit; boot `boot_hello.bin`
   headless; capture the CGA/text transcript; commit commands + output.
3. Boot `boot_hello.bin` on `ai_hw_engineer/system/ibm_pc_xt_5160`
   (`scripts/run_system.sh`); capture transcript; resolve any
   primary/secondary disagreement as a finding.
4. Implement `platform/scripts/run_image.sh` on the qualified oracle CLIs;
   define the transcript format in `plans/testplan.md` (then this plan shrinks).
5. Pin gcc-ia16 / Open Watcom / Capstone versions; add the Capstone
   8086-forbidden-opcode check as a committed script.
6. Update ai_sw_engineer#59 fields (Path, Start date, Status, Agent) — needs
   `gh` auth, unavailable in the session that wrote this.

## Gated on ai_engineer#60 (backend qualifies)

7. Rust smoke programs (`no_std`, boot + `.COM`): compile via the backend,
   `Capstone`/`ndisasm` check vs `cpu_abi.md` §2, boot on both oracles,
   compare against oracle transcripts (boxes 6–7).
8. crt0 + `#[panic_handler]` + first HAL impls in `platform/runtime/`.
9. MZ `.EXE` support only when a DOS-hosted title needs it.
10. Flip `report/final_report.md` to final; tick issue boxes with PR links.

## Non-goals

- Any `llvm-i8086` backend code (selection, MC, linker) — that is #60.
- Application/OS rewrites — those are ai_engineer#84's.
- The Rust whole-system emulator (#29) — parallel, non-blocking.
