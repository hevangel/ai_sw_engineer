#!/usr/bin/env python3
"""Execution test for i16 add/sub compiled by llc, run on external/chips.

Companion to exec_fn.py (i8). i16 args go in double-pairs D1 (P2:P3) and D2
(P4:P5); the i16 result comes back in D1 (abi.md sec.4.1/4.2). We inline the
compiled function body into a driver that presets the four pairs, runs the
body, and copies D1's four nibbles into RAM chip0 reg0 chars 0..3 so run_fn's
`result16` reconstructs the 16-bit value. Compared against the host.
"""
import os
import re
import subprocess
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BIN = os.path.join(ROOT, "install", "bin")
LLC = os.environ.get("LLC", os.path.join(BIN, "llc"))
LLVM_MC = os.environ.get("LLVM_MC", os.path.join(BIN, "llvm-mc"))
LLVM_OBJDUMP = os.environ.get("LLVM_OBJDUMP", os.path.join(BIN, "llvm-objdump"))
XCHECK = os.path.join(ROOT, "tools", "chips_xcheck")


def run(cmd, **kw):
    return subprocess.run(cmd, check=True, capture_output=True, text=True, **kw)


def compile_body(ir, func):
    with tempfile.NamedTemporaryFile("w", suffix=".ll", delete=False) as f:
        f.write(ir)
        ll = f.name
    s = run([LLC, "-mtriple=i4004", ll, "-o", "-"]).stdout
    os.unlink(ll)
    body, in_fn = [], False
    for ln in s.splitlines():
        t = ln.strip()
        if t.startswith(f"{func}:"):
            in_fn = True
            continue
        if not in_fn:
            continue
        if t.startswith(".") or t.startswith(";") or t == "" or t.endswith(":"):
            continue
        # Leaf (bbl) or soft-call return (abi.md 4.4.1: compiled functions
        # return via `jun __soft_ret`) ends the straight-line body.
        if t.startswith("bbl") or re.match(r"jun\s+__soft_ret\b", t):
            break
        body.append(t)
    if not body:
        raise RuntimeError(f"no body for {func}:\n{s}")
    return body


def parse_objdump_text(dump):
    data = bytearray()
    started = False
    for ln in dump.splitlines():
        if "Contents of section .text" in ln:
            started = True
            continue
        if not started:
            continue
        m = re.match(r"\s*([0-9a-fA-F]+)\s+((?:[0-9a-fA-F]{2}\s?){1,16})", ln)
        if not m:
            continue
        data += bytes.fromhex(m.group(2).replace(" ", ""))
    return bytes(data)


def build_rom(body, x, y):
    x_lo, x_hi = x & 0xFF, (x >> 8) & 0xFF
    y_lo, y_hi = y & 0xFF, (y >> 8) & 0xFF
    asm = ["    .text", "_start:"]
    asm.append(f"    fim p2, {x_lo}")
    asm.append(f"    fim p3, {x_hi}")
    asm.append(f"    fim p4, {y_lo}")
    asm.append(f"    fim p5, {y_hi}")
    asm += [f"    {ln}" for ln in body]
    # Result D1 = P2:P3 = r4(hi):r5(lo) | r6(hi):r7(lo). Store so run_fn's
    # result16 = (char3<<12|char2<<8|char1<<4|char0): char0=r5, char1=r4,
    # char2=r7, char3=r6.
    for ch, reg in [(0, "r5"), (1, "r4"), (2, "r7"), (3, "r6")]:
        asm.append(f"    fim p0, 0x{ch:02x}")
        asm.append("    src p0")
        asm.append(f"    ld {reg}")
        asm.append("    wrm")
    asm += ["halt:", "    jun halt"]
    src_text = "\n".join(asm) + "\n"
    with tempfile.TemporaryDirectory() as d:
        sp, op = os.path.join(d, "d.s"), os.path.join(d, "d.o")
        with open(sp, "w") as f:
            f.write(src_text)
        run([LLVM_MC, "-triple=i4004", "-filetype=obj", sp, "-o", op])
        dump = run([LLVM_OBJDUMP, "-s", "-j", ".text", op]).stdout
    return parse_objdump_text(dump)


def execute(rom, cycles=1000):
    with tempfile.NamedTemporaryFile("wb", suffix=".bin", delete=False) as f:
        f.write(rom)
        rp = f.name
    try:
        out = run(["cargo", "run", "--quiet", "--bin", "run_fn", "--", rp,
                   str(cycles)], cwd=XCHECK).stdout
    finally:
        os.unlink(rp)
    kv = {}
    for ln in out.splitlines():
        if "=" in ln:
            k, v = ln.split("=", 1)
            kv[k.strip()] = int(v.strip())
    return kv


ADD_IR = "define i16 @add16(i16 %x, i16 %y){ %r = add i16 %x, %y\n ret i16 %r }\n"
SUB_IR = "define i16 @sub16(i16 %x, i16 %y){ %r = sub i16 %x, %y\n ret i16 %r }\n"


def main():
    cases = [("add16", ADD_IR, lambda x, y: (x + y) & 0xFFFF),
             ("sub16", SUB_IR, lambda x, y: (x - y) & 0xFFFF)]
    inputs = [(0, 0), (1, 2), (0x00FF, 1), (0x0102, 0x0304), (0x1234, 0x1111),
              (0xFFFF, 1), (0x8000, 0x8000), (0x0A0B, 0x0506), (300, 500),
              (0xABCD, 0x1111)]
    fails = total = 0
    for name, ir, host in cases:
        body = compile_body(ir, name)
        for (x, y) in inputs:
            kv = execute(build_rom(body, x, y))
            got = kv.get("result16")
            want = host(x, y)
            total += 1
            ok = got == want
            fails += 0 if ok else 1
            print(f"{name}(0x{x:04x},0x{y:04x}) -> chips=0x{got:04x} "
                  f"host=0x{want:04x} {'OK' if ok else 'MISMATCH'}")
    print(f"\n{total - fails}/{total} i16 execution checks passed on external/chips")
    return 1 if fails else 0


if __name__ == "__main__":
    sys.exit(main())
