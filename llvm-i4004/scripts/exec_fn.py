#!/usr/bin/env python3
"""Execution test for llvm-i4004-compiled code on the REAL external/chips CPU.

Not a FileCheck test: it compiles a function with our `llc`, runs the compiled
MCS-4 instructions on chips' `mcs4::Board`, and compares the recovered result
against the same function computed on the host for many inputs.

Pipeline (all inside the ai-sw-engineer container):
  1. llc -mtriple=i4004 compiles the IR to .s.
  2. We extract the function body (the instruction lines between the label and
     its terminating `bbl`), which operates on P1/P2 per the i4004-C ABI.
  3. We wrap it in a flat driver at ROM 0: preset P1=a, P2=b (FIM), run the
     body (result lands in P1 = R2:R3), then copy R2:R3 into RAM chip0 reg0
     char1/char0 (SRC+WRM) and park in a self JUN loop.
  4. Assemble the driver with our llvm-mc, dump .text to a raw ROM image.
  5. tools/chips_xcheck `run_fn` executes the image on chips and prints the
     reconstructed result byte.
  6. Compare to the host computation.

The oracle is the host arithmetic (what the function means), and the device is
chips' real execution of OUR compiled bytes. A backend bug in the add/sub
nibble sequence shows up as a mismatch here, where a FileCheck test could not.

Usage: exec_fn.py            # runs the built-in add/sub test matrix
Exit status is nonzero on any mismatch.
"""
import os
import pathlib
import re
import subprocess
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))  # llvm-i4004
BIN = os.path.join(ROOT, "install", "bin")
LLC = os.environ.get("LLC", os.path.join(BIN, "llc"))
LLVM_MC = os.environ.get("LLVM_MC", os.path.join(BIN, "llvm-mc"))
LLVM_OBJDUMP = os.environ.get("LLVM_OBJDUMP", os.path.join(BIN, "llvm-objdump"))
XCHECK = os.path.join(ROOT, "tools", "chips_xcheck")


def contained(base, *parts):
    """Resolve parts under base and refuse paths that escape it (SAST guard)."""
    real_base = pathlib.Path(base).resolve()
    p = real_base.joinpath(*parts).resolve()
    if p != real_base and real_base not in p.parents:
        raise SystemExit(f"path escapes {real_base}: {parts}")
    return p


def run(cmd, **kw):
    return subprocess.run(cmd, check=True, capture_output=True, text=True, **kw)


def compile_body(ir, func):
    """Return the list of asm instruction lines of `func` (sans label/bbl)."""
    with tempfile.NamedTemporaryFile("w", suffix=".ll", delete=False) as f:
        f.write(ir)
        ll = f.name
    s = run([LLC, "-mtriple=i4004", ll, "-o", "-"]).stdout
    os.unlink(ll)
    # Grab lines after `<func>:` up to (not including) the terminating `bbl`.
    lines = s.splitlines()
    body = []
    in_fn = False
    for ln in lines:
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
        raise RuntimeError(f"no body extracted for {func}; asm was:\n{s}")
    return body


def build_rom(body, a, b):
    """Assemble a flat driver ROM: set P1=a,P2=b; run body; store R2:R3 to
    RAM chip0 reg0 char1/char0; self-loop. Returns raw bytes."""
    asm = []
    asm.append("    .text")
    asm.append("_start:")
    asm.append(f"    fim p1, {a}")   # P1 = arg a (R2:R3)
    asm.append(f"    fim p2, {b}")   # P2 = arg b (R4:R5)
    asm += [f"    {ln}" for ln in body]
    # Readout: result is in P1 = R2(hi):R3(lo). Store low nibble to char0, high
    # nibble to char1 of RAM chip0 reg0 (SRC pointer in P0 = R0:R1).
    asm.append("    fim p0, 0x00")   # pointer chip0 reg0 char0
    asm.append("    src p0")
    asm.append("    ld r3")          # ACC <- low nibble
    asm.append("    wrm")            # char0 <- low
    asm.append("    fim p0, 0x01")   # pointer char1
    asm.append("    src p0")
    asm.append("    ld r2")          # ACC <- high nibble
    asm.append("    wrm")            # char1 <- high
    asm.append("halt:")
    asm.append("    jun halt")       # park
    src_text = "\n".join(asm) + "\n"

    with tempfile.TemporaryDirectory() as d:
        sp = contained(d, "drv.s")
        op = contained(d, "drv.o")
        sp.write_text(src_text)
        run([LLVM_MC, "-triple=i4004", "-filetype=obj", sp, "-o", op])
        # Dump .text bytes with objdump -s (full-contents hex).
        dump = run([LLVM_OBJDUMP, "-s", "-j", ".text", op]).stdout
    return parse_objdump_text(dump), src_text


def parse_objdump_text(dump):
    """Parse `llvm-objdump -s` full-contents output into raw bytes."""
    data = bytearray()
    started = False
    for ln in dump.splitlines():
        m = re.match(r"\s*([0-9a-fA-F]+)\s+((?:[0-9a-fA-F]{2}\s?){1,16})", ln)
        if "Contents of section .text" in ln:
            started = True
            continue
        if not started:
            continue
        if not m:
            continue
        hexpart = m.group(2).replace(" ", "")
        try:
            data += bytes.fromhex(hexpart)
        except ValueError:
            pass
    return bytes(data)


def execute(rom, cycles=400):
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


ADD_IR = ("define i8 @add8(i8 %a, i8 %b) { %r = add i8 %a, %b\n ret i8 %r }\n")
SUB_IR = ("define i8 @sub8(i8 %a, i8 %b) { %r = sub i8 %a, %b\n ret i8 %r }\n")


def main():
    for tool in (LLC, LLVM_MC, LLVM_OBJDUMP):
        if not (os.path.exists(tool) or tool in ("llc", "llvm-mc", "llvm-objdump")):
            print(f"ERROR: missing tool {tool}", file=sys.stderr)
            return 2

    cases = [
        ("add8", ADD_IR, lambda a, b: (a + b) & 0xFF),
        ("sub8", SUB_IR, lambda a, b: (a - b) & 0xFF),
    ]
    inputs = [(0, 0), (1, 2), (9, 8), (15, 1), (200, 55), (0xFF, 1),
              (0x12, 0x34), (100, 100), (0x80, 0x80), (7, 250)]

    fails = 0
    total = 0
    for name, ir, host in cases:
        body = compile_body(ir, name)
        for (a, b) in inputs:
            rom, _ = build_rom(body, a, b)
            kv = execute(rom)
            got = kv.get("result")
            want = host(a, b)
            total += 1
            ok = got == want
            if not ok:
                fails += 1
            print(f"{name}({a:3d},{b:3d}) -> chips={got!s:>4}  host={want:3d}  "
                  f"{'OK' if ok else 'MISMATCH'}")
    print(f"\n{total - fails}/{total} execution checks passed on external/chips")
    return 1 if fails else 0


if __name__ == "__main__":
    sys.exit(main())
