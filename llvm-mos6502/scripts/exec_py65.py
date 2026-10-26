#!/usr/bin/env python3
"""Execute the llvm-mos6502 smoke image on the vendored py65 oracle and
compare the printed window with the independent expected values.

Runs INSIDE the ai-sw-engineer container, from the repository root, after
scripts/build.sh:

    llvm-mos6502/scripts/exec_py65.py

py65 (BSD/MIT, vendored by ai_hw_engineer as its 6502 oracle at
$AI_HW_ENGINEER/design/mos_6502/references/, renamed with underscores) is an
independent 6502 implementation — neither llvm-mos nor the project's own
code. The vendored files use dotted imports for underscore-renamed modules;
a small sys.modules shim bridges them.
"""
import hashlib
import importlib.util
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))          # scripts/
BACKEND = os.path.dirname(HERE)                            # llvm-mos6502/
ROOT = os.path.dirname(BACKEND)                            # repo root
IMAGE = os.path.join(BACKEND, "build", "smoke.bin")
REFS = os.path.join(
    os.environ.get("AI_HW_ENGINEER", os.path.join(ROOT, "..", "ai_hw_engineer")),
    "design", "mos_6502", "references")

sys.path.insert(0, HERE)
from expected import WINDOW, RESULT_BASE, DONE_ADDR, DONE_VALUE, window  # noqa: E402


def load_py65():
    # shim: py65.utils.conversions -> vendored py65_utils_conversions.py, etc.
    import types
    utils = types.ModuleType("py65.utils")
    conv = importlib.util.spec_from_file_location(
        "py65.utils.conversions", os.path.join(REFS, "py65_utils_conversions.py"))
    devs = importlib.util.spec_from_file_location(
        "py65.utils.devices", os.path.join(REFS, "py65_utils_devices.py"))
    utils.conversions = importlib.util.module_from_spec(conv)
    utils.devices = importlib.util.module_from_spec(devs)
    conv.loader.exec_module(utils.conversions)
    devs.loader.exec_module(utils.devices)
    py65 = types.ModuleType("py65")
    py65.utils = utils
    sys.modules["py65"] = py65
    sys.modules["py65.utils"] = utils
    sys.modules["py65.utils.conversions"] = utils.conversions
    sys.modules["py65.utils.devices"] = utils.devices
    spec = importlib.util.spec_from_file_location(
        "py65_mpu6502", os.path.join(REFS, "py65_mpu6502.py"))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod.MPU


def trap_addr():
    """__trap address from the build's symbol table (never hard-coded)."""
    with open(os.path.join(BACKEND, "build", "symbols.txt")) as fh:
        for line in fh:
            parts = line.split()
            if len(parts) == 3 and parts[2] == "__trap":
                return int(parts[0], 16)
    raise SystemExit("__trap not in build/symbols.txt (run scripts/build.sh)")


def main():
    image = open(IMAGE, "rb").read()
    assert len(image) == 32768, "run scripts/build.sh first"
    trap = trap_addr()

    mpu = load_py65()()
    mpu.memory = bytearray(0x10000)
    mpu.memory[0x8000:0x10000] = image
    mpu.addrMask = 0xFFFF
    mpu.start_pc = None  # reset() must read the RESET vector, not a fixed PC
    mpu.reset()

    cap = 5_000_000
    steps = 0
    trace = []
    while mpu.memory[DONE_ADDR] != DONE_VALUE:
        trace.append(mpu.pc)
        if len(trace) > 16:
            trace.pop(0)
        mpu.step()
        steps += 1
        if steps >= cap or mpu.pc == trap:
            raise SystemExit(
                f"FAIL: diverged before sentinel after {steps} instructions "
                f"(pc={mpu.pc:#06x}); last PCs: "
                + " ".join(f"{p:#06x}" for p in trace))

    got = list(mpu.memory[RESULT_BASE:RESULT_BASE + WINDOW])
    want = window()
    if got != want:
        for i, (g, w) in enumerate(zip(got, want)):
            if g != w:
                print(f"FAIL: window[{i}] got {g:#04x} want {w:#04x}")
                break
        raise SystemExit(f"FAIL after {steps} instructions")
    print(f"PASS: py65 window matches the independent expected values "
          f"({steps} instructions)")
    print(f"registers: A={mpu.a:#04x} X={mpu.x:#04x} Y={mpu.y:#04x} "
          f"SP={mpu.sp:#04x} PC={mpu.pc:#06x}")


if __name__ == "__main__":
    main()
