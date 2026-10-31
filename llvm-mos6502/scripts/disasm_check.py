#!/usr/bin/env python3
"""Independent opcode verification of the llvm-mos6502 smoke image.

Runs INSIDE the ai-sw-engineer container, from the repository root, after
scripts/build.sh:

    llvm-mos6502/scripts/disasm_check.py

Checks (independent of llvm-mos's own objdump — the opcode table is the
vendored py65, BSD/MIT, ai_hw_engineer's 6502 oracle):

  1. RESET vector == the crt0 `_start` address from the ELF symbol table;
  2. the linked .text walks as an unbroken sequence of DOCUMENTED NMOS
     opcodes (py65 raises no '???' entries; the stream stays in sync via
     per-addressing-mode instruction lengths);
  3. the instruction mix includes every kind the smoke source requires
     (loads/stores, ALU, shifts/rotates, compares, jumps/subroutines), and
     the park/trap loops decode as JMP-to-self.
"""
import importlib.util
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
BACKEND = os.path.dirname(HERE)
ROOT = os.path.dirname(BACKEND)
BUILD = os.path.join(BACKEND, "build")
ELF = os.path.join(BUILD, "target", "mos-unknown-none", "release", "smoke")
REFS = os.path.join(
    os.environ.get("AI_HW_ENGINEER", os.path.join(ROOT, "..", "ai_hw_engineer")),
    "design", "mos_6502", "references")

# py65 addressing mode -> instruction length in bytes (mode names exactly as
# the vendored py65_mpu6502.py's @instruction decorators spell them)
MODE_LENGTH = {
    "imp": 1, "acc": 1, "imm": 2, "zpg": 2, "zpx": 2, "zpy": 2,
    "inx": 2, "iny": 2, "rel": 2,
    "abs": 3, "abx": 3, "aby": 3, "ind": 3,
}

REQUIRED_OPS = ("LDA", "STA", "ADC", "SBC", "EOR", "LSR", "ROR", "CMP",
                "INX", "JMP", "JSR", "TXS", "RTS")


def load_py65_mpu():
    import types
    utils = types.ModuleType("py65.utils")
    for name in ("conversions", "devices"):
        spec = importlib.util.spec_from_file_location(
            f"py65.utils.{name}", os.path.join(REFS, f"py65_utils_{name}.py"))
        mod = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(mod)
        setattr(utils, name, mod)
        sys.modules[f"py65.utils.{name}"] = mod
    py65 = types.ModuleType("py65")
    py65.utils = utils
    sys.modules["py65"] = py65
    sys.modules["py65.utils"] = utils
    spec = importlib.util.spec_from_file_location(
        "py65_mpu6502", os.path.join(REFS, "py65_mpu6502.py"))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod.MPU


def read_text_bytes():
    out = subprocess.run(
        ["llvm-objdump", "-s", "-j", ".text", ELF],
        capture_output=True, text=True, check=True).stdout
    data = {}
    for line in out.splitlines():
        m = re.match(r"\s+([0-9a-f]{4,})\s+((?:[0-9a-f]{2,8}\s+){1,4})", line)
        if m:
            addr = int(m.group(1), 16)
            for chunk in m.group(2).split():
                for k in range(0, len(chunk), 2):
                    data[addr] = int(chunk[k:k + 2], 16)
                    addr += 1
    start, end = min(data), max(data) + 1
    return start, bytes(data[a] for a in range(start, end))


def symbol(name):
    out = subprocess.run(["llvm-nm", ELF], capture_output=True, text=True,
                         check=True).stdout
    for line in out.splitlines():
        parts = line.split()
        if len(parts) == 3 and parts[2] == name:
            return int(parts[0], 16)
    raise SystemExit(f"symbol {name} not found in {ELF}")


def main():
    image = open(os.path.join(BUILD, "smoke.bin"), "rb").read()
    assert len(image) == 32768

    # 1. reset vector -> _start (the flat image starts at 0x8000)
    reset = image[0x7FFC] | (image[0x7FFD] << 8)
    start = symbol("_start")
    assert reset == start, f"RESET vector {reset:#06x} != _start {start:#06x}"
    print(f"ok: RESET vector {reset:#06x} == _start")

    # 2. walk .text as documented opcodes only
    table = load_py65_mpu().disassemble
    tstart, tdata = read_text_bytes()
    seen = set()
    pc = tstart
    while pc < tstart + len(tdata):
        opcode = image[pc - 0x8000]
        template, mode = table[opcode]
        assert template != "???", \
            f"undocumented opcode {opcode:#04x} at {pc:#06x}"
        seen.add(template)
        try:
            length = MODE_LENGTH[mode]
        except KeyError:
            raise SystemExit(f"unknown py65 addressing mode {mode!r} "
                             f"for opcode {opcode:#04x}")
        pc += length
    print(f"ok: {len(tdata)} .text bytes walk as documented NMOS opcodes "
          f"({tstart:#06x}..{pc:#06x})")

    # 3. required instruction kinds + self-loop traps
    missing = [op for op in REQUIRED_OPS if op not in seen]
    assert not missing, f"missing instruction kinds: {missing}"
    print("ok: disassembly covers the smoke program's required instruction kinds")
    park = symbol("park")
    idx = park - 0x8000
    assert image[idx:idx + 3] == bytes([0x4C, park & 0xFF, park >> 8]), \
        "park is not JMP-to-self"
    trap = symbol("__trap")
    idx = trap - 0x8000
    assert image[idx:idx + 3] == bytes([0x4C, trap & 0xFF, trap >> 8]), \
        "__trap is not JMP-to-self"
    print("ok: park and __trap decode as JMP-to-self loops")
    print("PASS: independent opcode verification")


if __name__ == "__main__":
    main()
