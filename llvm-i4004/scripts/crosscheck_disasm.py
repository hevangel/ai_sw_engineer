#!/usr/bin/env python3
"""Thin driver for the llvm-i4004 disassembler cross-check.

HISTORY: the previous version of this file (PR #8) claimed to verify the
disassembler against "two independent decoders". It did not: it compared
llvm-mc's mnemonic against ``oracle_dispatch()`` — a hand-written Python opcode
table in this same file — and only regex-checked that external/chips / the RTL
*source* contained a few expected tokens. No operand (register, pair, condition,
data, address) was ever checked, and nothing was executed. That is our own
table checked against itself.

This file is now a THIN driver. The real check lives in the Rust tool
``llvm-i4004/tools/chips_xcheck`` which, for every 4004 encoding:

  1. disassembles the bytes with OUR llvm-mc (mnemonic + operands),
  2. predicts the machine effect from THAT disassembly using MCS-4 manual
     semantics (spec/isa.md), parameterised by the operands we printed,
  3. EXECUTES the same bytes on the external/chips emulator (its real
     ``mcs4::Board``; read & run only, never copied) and recovers the effect
     through the chip's own instructions,
  4. asserts prediction == observation.

A wrong operand field or a swapped mnemonic makes step 2 disagree with step 3 —
a FINDING. The oracle is chips' actual execution, not a table. See
``tools/chips_xcheck`` and ``report/final_report.md`` §disassembler-crosscheck.

Usage (inside the ai-sw-engineer container; llvm-mc from the build volume or
install/bin):
    llvm-i4004/scripts/crosscheck_disasm.py
    LLVM_MC=/path/to/llvm-mc llvm-i4004/scripts/crosscheck_disasm.py

Exit status: 0 = no disagreement, 1 = at least one FINDING, 2 = setup error.
"""
import os
import shutil
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
BACKEND = os.path.dirname(HERE)
TOOL = os.path.join(BACKEND, "tools", "chips_xcheck")


def find_llvm_mc():
    if os.environ.get("LLVM_MC"):
        return os.environ["LLVM_MC"]
    for c in (
        "/llvm-build/llvm-obj/bin/llvm-mc",
        os.path.join(BACKEND, "install", "bin", "llvm-mc"),
    ):
        if os.path.exists(c):
            return c
    return shutil.which("llvm-mc")


def main():
    if not shutil.which("cargo"):
        print("ERROR: cargo not found. Run inside the ai-sw-engineer container "
              "(scripts/docker_run.sh).", file=sys.stderr)
        return 2

    mc = find_llvm_mc()
    if not mc or not os.path.exists(mc):
        print(f"ERROR: i4004 llvm-mc not found (looked at LLVM_MC, "
              f"/llvm-build/llvm-obj/bin, install/bin). Build it with "
              f"scripts/build_llvm.sh first.", file=sys.stderr)
        return 2

    env = dict(os.environ)
    env["LLVM_MC"] = mc
    print(f"==> building chips_xcheck (cargo, release off for speed)")
    build = subprocess.run(
        ["cargo", "build", "--quiet"], cwd=TOOL, env=env
    )
    if build.returncode != 0:
        print("ERROR: chips_xcheck failed to build.", file=sys.stderr)
        return 2

    # Executor self-test first (cheap, no llvm-mc dependency).
    smoke = subprocess.run(
        ["cargo", "run", "--quiet", "--", "--smoke"], cwd=TOOL, env=env
    )
    if smoke.returncode != 0:
        print("ERROR: executor self-test (--smoke) failed; the chips harness is "
              "not trustworthy, so the cross-check result would be meaningless.",
              file=sys.stderr)
        return 2

    # The real differential cross-check.
    run = subprocess.run(["cargo", "run", "--quiet"], cwd=TOOL, env=env)
    return run.returncode


if __name__ == "__main__":
    sys.exit(main())
