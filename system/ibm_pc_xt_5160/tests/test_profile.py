#!/usr/bin/env python3
"""Consistency of the XT 5160 compiler profile (ai_sw_engineer#59). No compiler or emulator needed:

    python3 -I system/ibm_pc_xt_5160/tests/test_profile.py

The profile reuses the 5150 profile and the shared llvm-i8086 backend, so the checks are that it
says so and stays in step with them:
1. toolchain/spec/profile.md states the backend's data layout (llvm-i8086/spec/abi.md D1) exactly,
   and the Rust route of llvm-i8086/scripts/build_rust.sh uses the same string;
2. no second, provisional XT target or XT-only linker script exists (the 2026-10-31 review);
3. build_profile.sh links with i8086-ld and the 5150 platform's runtime and linker scripts, and
   those files exist;
4. the expected screen states the ROM sum of the pinned BIOS hash and the RAM size of the profile.
Exit 0 = all pass.
"""
import re
import sys
from pathlib import Path

XT = Path(__file__).resolve().parent.parent
ROOT = XT.parent.parent
fails = []


def check(name, cond, detail=""):
    print(("PASS " if cond else "FAIL ") + name + ("" if cond else f" -- {detail}"))
    if not cond:
        fails.append(name)


abi = (ROOT / "llvm-i8086/spec/abi.md").read_text(encoding="utf-8")
m = re.search(r"D1\. LLVM data layout string: `([^`]+)`", abi)
d1 = m.group(1) if m else None
check("abi.md states D1", d1 is not None)
profile = (XT / "toolchain/spec/profile.md").read_text(encoding="utf-8")
check("profile.md states D1 exactly", d1 is not None and f"`{d1}`" in profile)
br = (ROOT / "llvm-i8086/scripts/build_rust.sh").read_text(encoding="utf-8")
check("build_rust.sh retargets to D1", d1 is not None and f'DATA_LAYOUT="{d1}"' in br)

check("no provisional XT target JSON", not (XT / "rust/i8086-xt-none.json").exists())
check("no XT-only linker script", not (XT / "platform/link.ld").exists())

build = (XT / "toolchain/scripts/build_profile.sh").read_text(encoding="utf-8")
check("build_profile.sh uses i8086-ld", "i8086-ld" in build)
check("build_profile.sh uses the 5150 platform", "PLAT=system/ibm_pc/platform" in build)
for f in ["runtime/crt0.s", "runtime/crt0_com.s", "runtime/dos.s", "runtime/hal.s",
          "link.ld", "link_com.ld", "link_exe_seg.ld"]:
    check(f"system/ibm_pc/platform/{f} exists", (ROOT / "system/ibm_pc/platform" / f).exists())

exp = (XT / "toolchain/expected/xt_info.txt").read_text(encoding="utf-8")
check("expected RAM is the profile's 256 KiB", "\nram = 256 KiB\n" in exp and "256 KiB" in profile)
check("expected names the pinned BIOS", "2798f8a9" in exp and "2798f8a9a7dcab651ede40373d35269ad573b75acf811115c263e034ef80a1f2" in profile)

print(f"{len(fails)} failed" if fails else "all passed")
sys.exit(1 if fails else 0)
