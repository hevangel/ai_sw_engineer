# Test plan: XT 5160 toolchain profile (ai_sw_engineer#59)

Levels run bottom-up; each names its command, oracle, and expected evidence.
"Committed" = in this repo; "bring-up" = ordered future work.

## L0 — profile self-consistency (committed, green)

- `python3 system/ibm_pc_xt_5160/tests/test_profile.py` → 22/22 PASS.
- Covers: target JSON parses and matches `spec/cpu_abi.md` §5 verbatim,
  `link.ld` regions, baseline source shape, ledger IDs, backend-ownership
  pointer. Needs no toolchain beyond Python 3.

## L1 — baseline image build (bring-up)

- `system/ibm_pc_xt_5160/bringup/build.sh` → 512-byte `boot_hello.bin`,
  `55 AA` signature, committed sha256. Needs NASM (pin in Docker image).
- `ndisasm -b 16` / Capstone `CS_MODE_16` decode matches `expected.txt`.

## L2 — oracle boot matrix (bring-up)

Same `boot_hello.bin` on:

| Oracle | Command (to qualify) | Expected |
|---|---|---|
| Primary: XT RTL | `ai_hw_engineer/system/ibm_pc_xt_5160/scripts/run_system.sh` + boot A: | `XT-BOOT-HELLO` at CGA 0,0; halted, IF=0 |
| Secondary: MartyPC | `external/martypc` headless run at pinned commit | byte-identical screen + halt state |

Disagreement between oracles is a finding, not noise (AGENTS.md). Transcript
format: reuse the micral_n `run_image.sh` convention (`< hex bytes` outputs)
once `run_image.sh` is implemented; expected transcripts live next to the
image under `bringup/`.

## L3 — Rust smoke compile + decode (gated on ai_engineer#60)

- Compile representative `no_std` smoke programs through the qualified
  backend; disassemble with Capstone/`ndisasm`; assert: 8088-only opcodes
  (no 66h/67h, no 186+), relocations resolve, entry/placement per
  `platform/link.ld`; forbidden-opcode script fails the run on violation.

## L4 — Rust smoke execution (gated on ai_engineer#60)

- Boot/run L3 images on both L2 oracles; diff observed output vs committed
  oracle transcripts. Host-native runs and successful compilation alone
  never satisfy this level (issue box 7).

## L5 — whole-program size gate (from the first function that lowers)

- Every landing that touches this profile or its backend records
  per-function `.text`, largest function, and image size vs the §8 budget
  (AGENTS.md whole-program size gate), same measurement as the previous row.
