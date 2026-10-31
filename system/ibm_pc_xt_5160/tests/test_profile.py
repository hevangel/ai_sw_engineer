#!/usr/bin/env python3
"""Profile regression tests: the toolchain-profile artifacts must agree with
each other (ai_sw_engineer#59, box 8, backend-independent part).

Usage: python3 system/ibm_pc_xt_5160/tests/test_profile.py
Exit 0 = all pass. No compiler, emulator, or NASM needed.
"""
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
fails = []


def check(name, cond, detail=""):
    print(("PASS " if cond else "FAIL ") + name + ("" if cond else f" -- {detail}"))
    if not cond:
        fails.append(name)


def read(rel):
    return (ROOT / rel).read_text(encoding="utf-8")


# 1. Target JSON parses and carries the spec'd data layout verbatim.
spec = read("spec/cpu_abi.md")
m = re.search(r"^```\n(e-[^\n]*)\n```", spec, re.M)
spec_layout = m.group(1).strip() if m else None
check("cpu_abi.md carries a data-layout block", spec_layout is not None)

tj = json.loads(read("rust/i8086-xt-none.json"))
check("target JSON parses", isinstance(tj, dict))
check("target JSON data-layout == spec sec.5", tj.get("data-layout") == spec_layout,
      f"json={tj.get('data-layout')!r} spec={spec_layout!r}")
check("target-pointer-width == 16", tj.get("target-pointer-width") == 16)
check("target-endian == little", tj.get("target-endian") == "little")
check("panic-strategy == abort", tj.get("panic-strategy") == "abort")
check("disable-redzone (XT-A3)", tj.get("disable-redzone") is True)
check("arch/llvm-target name the i8086",
      tj.get("arch") == "i8086" and str(tj.get("llvm-target")).startswith("i8086-"))
check("target JSON marked provisional", "PROVISIONAL" in tj.get("_comment", ""))

# 2. Linker script regions match the memory map budgets.
ld = read("platform/link.ld")
boot = re.search(r"boot\s*\(rx\)\s*:\s*ORIGIN\s*=\s*(0x[0-9A-Fa-f]+)\s*,\s*LENGTH\s*=\s*(\d+)", ld)
check("link.ld boot ORIGIN=0x7C00 LENGTH=510",
      boot is not None and int(boot.group(1), 16) == 0x7C00 and int(boot.group(2)) == 510,
      "boot region missing/wrong")
com = re.search(r"com\s*\(rx\)\s*:\s*ORIGIN\s*=\s*(0x[0-9A-Fa-f]+)", ld)
check("link.ld com ORIGIN=0x0100",
      com is not None and int(com.group(1), 16) == 0x100, "com region missing/wrong")

# 3. Baseline boot sector source has the required shape (assembly itself needs NASM).
asm = read("bringup/boot_hello.asm")
for token in ["BITS 16", "ORG 0x7C00", "int 0x10", "XT-BOOT-HELLO",
              "times 510-($-$$) db 0", "dw 0xAA55"]:
    check(f"boot_hello.asm contains {token!r}", token in asm)

# 4. Assumption ledger IDs exist with the same meaning (AGENTS.md rule).
for aid in ["XT-A1", "XT-A2", "XT-A3", "XT-A4"]:
    check(f"ledger ID {aid} defined in cpu_abi.md", aid in spec)

# 5. Audit names the blocking backend without claiming it exists.
audit = read("spec/toolchain_audit.md")
check("audit references ai_engineer#60 as owner of llvm-i8086",
      "ai_engineer#60" in audit and "llvm-i8086" in audit)

print(f"\n{len(fails)} failure(s)")
sys.exit(1 if fails else 0)
