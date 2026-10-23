#!/usr/bin/env python3
"""Execution test for i8/i16 compares (all ten predicates) on external/chips.

Companion to exec_fn.py / exec_i16.py. Every `icmp` predicate is compiled by
our llc in two shapes -- a value (`zext (icmp ..)`, which lowers through
SELECT_CC) and a branch (`br (icmp ..)`, which lowers through BR_CC) -- and run
on chips over a sign-boundary value matrix. The oracle is host integer
comparison of the same values, so a signed relation lowered as unsigned (the
pre-fix mapCC) fails here on every mixed-sign pair.

Unlike exec_i16.py the WHOLE llc output is assembled (labels, frame spills and
all) behind a small driver:

    _start:  fim p.. (args) ; jms <fn>
    __soft_ret:                       ; BBL (or a soft-call return) lands here
             store P1 (R2:R3) -> RAM chip0 reg0 char1:char0 ; park
    __frame_<fn> = 0x40, __framebank_<fn> = 0   (spills in RAM chip1, bank 0)

`.globl` is stripped so every label is local and llvm-mc resolves all fixups
itself. The image is assembled ONCE per function with placeholder FIM
immediates which are then patched per input (FIM = [0x20 | pair<<1, imm]).

Usage: exec_cmp.py        (exit status nonzero on any mismatch)
"""
import itertools
import json
import os
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
CYCLES = 4000

PREDS = {
    "eq": lambda a, b: a == b, "ne": lambda a, b: a != b,
    "ult": lambda a, b: a < b, "ule": lambda a, b: a <= b,
    "ugt": lambda a, b: a > b, "uge": lambda a, b: a >= b,
    "slt": lambda a, b: a < b, "sle": lambda a, b: a <= b,
    "sgt": lambda a, b: a > b, "sge": lambda a, b: a >= b,
}
VALUES = {
    8: [-128, -127, -2, -1, 0, 1, 2, 126, 127],
    16: [-32768, -32767, -256, -129, -128, -1, 0, 1, 127, 128, 255, 256,
         32766, 32767],
}


# Constants for the compare-with-constant cases (taken modulo the width):
# 0 and -1 exercise the sign tests and the c == 0 fallbacks.
CONSTS = [0, 1, 2, 9, 0x7F, 0x80, 0xFF, 0x100, 0x7FFF, 0x8000, 0xFFFF, 0x1234]


def run(cmd, **kw):
    return subprocess.run(cmd, check=True, capture_output=True, text=True, **kw)


def ir_for(width, pred, shape, fn):
    t = f"i{width}"
    if shape == "val":
        return (f"define i8 @{fn}({t} %x, {t} %y) {{\n"
                f"  %c = icmp {pred} {t} %x, %y\n"
                f"  %r = zext i1 %c to i8\n  ret i8 %r\n}}\n")
    return (f"define i8 @{fn}({t} %x, {t} %y) {{\n"
            f"  %c = icmp {pred} {t} %x, %y\n"
            f"  br i1 %c, label %t, label %f\n"
            f"t:\n  ret i8 1\nf:\n  ret i8 0\n}}\n")


def compile_fn(ir):
    with tempfile.NamedTemporaryFile("w", suffix=".ll", delete=False) as f:
        f.write(ir)
        ll = f.name
    try:
        s = run([LLC, "-mtriple=i4004", ll, "-o", "-"]).stdout
    finally:
        os.unlink(ll)
    keep = []
    for ln in s.splitlines():
        t = ln.strip()
        if t.startswith((".globl", ".type", ".size", ".file", ".section")):
            continue
        keep.append(ln)
    return "\n".join(keep)


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
        if m:
            data += bytes.fromhex(m.group(2).replace(" ", ""))
    return bytes(data)


def build_image(fn, body, width, ret_width=8):
    # Argument pairs (abi.md sec.4.1): i8 x,y in P1,P2; i16 x in D1=P2:P3,
    # y in D2=P4:P5 (low byte in the lower pair). Result: i8 in P1 (R2:R3),
    # i16 in D1 (P2 = R4:R5 low byte, P3 = R6:R7 high byte) -> chars 0..3.
    pairs = [1, 2] if width == 8 else [2, 3, 4, 5]
    asm = ["    .text", "_start:"]
    asm += [f"    fim p{p}, 0" for p in pairs]
    # JMS/BBL convention (default); a -i4004-softcall build returns with
    # `jun __soft_ret`, which lands on the same instruction.
    asm.append(f"    jms {fn}")
    asm.append("__soft_ret:")
    ret = [(0, "r3"), (1, "r2")] if ret_width == 8 else \
        [(0, "r5"), (1, "r4"), (2, "r7"), (3, "r6")]
    for ch, reg in ret:
        asm += [f"    fim p0, 0x{ch:02x}", "    src p0", f"    ld {reg}", "    wrm"]
    asm += ["halt:", "    jun halt"]
    asm += [f"__frame_{fn} = 0x40", f"__framebank_{fn} = 0", body]
    with tempfile.TemporaryDirectory() as d:
        sp, op = os.path.join(d, "t.s"), os.path.join(d, "t.o")
        with open(sp, "w") as f:
            f.write("\n".join(asm) + "\n")
        run([LLVM_MC, "-triple=i4004", "-filetype=obj", sp, "-o", op])
        rel = run([LLVM_OBJDUMP, "-r", op]).stdout
        rom = bytearray(parse_objdump_text(
            run([LLVM_OBJDUMP, "-s", "-j", ".text", op]).stdout))
    # JCN is page-local; keep the whole test inside ROM page 0 so a far branch
    # can never silently wrap.
    if len(rom) > 0xFE:
        raise RuntimeError(f"{fn}: image {len(rom)} B exceeds ROM page 0")
    # .text is placed at ROM 0, so resolving the (section-relative) fixups is
    # a straight patch: ADDR12 = JUN/JMS 12-bit target, PAGE8 = in-page byte.
    for ln in rel.splitlines():
        if "R_I4004_" not in ln:
            continue
        m = re.match(r"([0-9a-f]+)\s+(R_I4004_\w+)\s+\.text(?:\+0x([0-9a-f]+))?$",
                     ln.strip())
        if not m:
            raise RuntimeError(f"{fn}: relocation not against .text: {ln}")
        off, kind, val = int(m.group(1), 16), m.group(2), int(m.group(3) or "0", 16)
        if kind == "R_I4004_ADDR12":
            rom[off] = (rom[off] & 0xF0) | (val >> 8)
            rom[off + 1] = val & 0xFF
        elif kind == "R_I4004_PAGE8":
            rom[off] = val & 0xFF
        else:
            raise RuntimeError(f"{fn}: unexpected relocation {kind}")
    return rom, pairs


def patch_args(rom, pairs, width, x, y):
    img = bytearray(rom)
    mask = (1 << width) - 1
    x &= mask
    y &= mask
    if width == 8:
        vals = [x, y]
    else:
        vals = [x & 0xFF, x >> 8, y & 0xFF, y >> 8]
    for i, (p, v) in enumerate(zip(pairs, vals)):
        assert img[2 * i] == 0x20 | (p << 1), "driver FIM layout changed"
        img[2 * i + 1] = v
    return bytes(img)


def execute(runner, rom, key="result"):
    with tempfile.NamedTemporaryFile("wb", suffix=".bin", delete=False) as f:
        f.write(rom)
        rp = f.name
    try:
        out = run([runner, rp, str(CYCLES)]).stdout
    finally:
        os.unlink(rp)
    for ln in out.splitlines():
        if ln.startswith(key + "="):
            return int(ln.split("=", 1)[1])
    raise RuntimeError(f"run_fn printed no {key}")


def build_runner():
    """Build chips_xcheck's run_fn once; return the binary path."""
    run(["cargo", "build", "--quiet", "--release", "--bin", "run_fn"], cwd=XCHECK)
    meta = run(["cargo", "metadata", "--format-version", "1", "--no-deps"],
               cwd=XCHECK).stdout
    return os.path.join(json.loads(meta)["target_directory"], "release", "run_fn")


def main():
    runner = build_runner()
    fails = total = 0
    for width, (pred, host), shape in itertools.product(
            (8, 16), PREDS.items(), ("val", "br")):
        fn = f"cmp_{pred}{width}_{shape}"
        rom, pairs = build_image(fn, compile_fn(ir_for(width, pred, shape, fn)),
                                 width)
        mask = (1 << width) - 1
        bad = 0
        for x, y in itertools.product(VALUES[width], repeat=2):
            a, b = (x, y) if pred.startswith("s") or pred in ("eq", "ne") \
                else (x & mask, y & mask)
            want = int(host(a, b))
            got = execute(runner, patch_args(rom, pairs, width, x, y))
            total += 1
            if got != want:
                bad += 1
                if bad <= 4:
                    print(f"  MISMATCH {fn}({x},{y}): chips={got} host={want}")
        fails += bad
        print(f"{fn:24s} {len(rom):4d} B  "
              f"{len(VALUES[width])**2 - bad}/{len(VALUES[width])**2}")
    # Compare-with-constant (BRCC*ri via I4004ImmCompare: LDM c; SUB x, the
    # ULT/UGE -> ULE/UGT c-1 rewrite, NEG/NONNEG sign tests) and compares of
    # two zero-extended nibbles (the used-nibble shortcut: only the low
    # nibble is compared).
    for width, (pred, host), c in itertools.product(
            (8, 16), PREDS.items(), CONSTS):
        t = f"i{width}"
        mask = (1 << width) - 1
        cs = c & mask
        if pred.startswith("s"):
            cs = cs - (1 << width) if cs & (1 << (width - 1)) else cs
        fn = f"cmpi_{pred}{width}_{c & mask:x}"
        ir = (f"define i8 @{fn}({t} %x, {t} %y) {{\n"
              f"  %c = icmp {pred} {t} %x, {cs}\n"
              f"  br i1 %c, label %t, label %f\n"
              f"t:\n  ret i8 1\nf:\n  ret i8 0\n}}\n")
        rom, pairs = build_image(fn, compile_fn(ir), width)
        bad = 0
        for x in VALUES[width]:
            # VALUES are signed; unsigned/equality relations see the bit pattern.
            if pred.startswith("s"):
                want = int(host(x, cs))
            else:
                want = int(host(x & mask, c & mask))
            got = execute(runner, patch_args(rom, pairs, width, x, 0))
            total += 1
            if got != want:
                bad += 1
                if bad <= 4:
                    print(f"  MISMATCH {fn}({x}): chips={got} host={want}")
        fails += bad
    for (pred, host), shape in itertools.product(PREDS.items(), ("val", "br")):
        if pred.startswith("s"):
            continue
        fn = f"cmpn_{pred}_{shape}"
        tail = ("  %r = zext i1 %c to i8\n  ret i8 %r\n}\n" if shape == "val" else
                "  br i1 %c, label %t, label %f\nt:\n  ret i8 1\nf:\n  ret i8 0\n}\n")
        ir = (f"define i8 @{fn}(i8 %x, i8 %y) {{\n"
              f"  %a = and i8 %x, 15\n  %b = and i8 %y, 15\n"
              f"  %c = icmp {pred} i8 %a, %b\n" + tail)
        rom, pairs = build_image(fn, compile_fn(ir), 8)
        bad = 0
        for x, y in itertools.product(range(0, 256, 17), range(3, 256, 23)):
            want = int(host(x & 15, y & 15))
            got = execute(runner, patch_args(rom, pairs, 8, x, y))
            total += 1
            if got != want:
                bad += 1
                if bad <= 4:
                    print(f"  MISMATCH {fn}({x},{y}): chips={got} host={want}")
        fails += bad
    # A 4002 character read then tested against a constant (BRRAM: the read
    # and the EQ/NE test fused, ACC compared directly). The function stores
    # %v at RAM address %a first, so the read sees a known value.
    for (kind, wr_fn, rd_fn), pred, c in itertools.product(
            (("main", "__ram_wr", "__ram_rd"), ("s3", "__ram_ws3", "__ram_rs3")),
            ("eq", "ne"), (0, 3, 9, 15)):
        fn = f"cmpr_{kind}_{pred}_{c}"
        ir = (f"declare void @{wr_fn}(i8, i8)\ndeclare i8 @{rd_fn}(i8)\n"
              f"define i8 @{fn}(i8 %a, i8 %v) {{\n"
              f"  call void @{wr_fn}(i8 %a, i8 %v)\n"
              f"  %r = call i8 @{rd_fn}(i8 %a)\n"
              f"  %c = icmp {pred} i8 %r, {c}\n"
              f"  br i1 %c, label %t, label %f\n"
              f"t:\n  ret i8 1\nf:\n  ret i8 0\n}}\n")
        body = compile_fn(ir)
        if "rd3" not in body and "rdm" not in body:
            raise RuntimeError(f"{fn}: no RAM read emitted:\n{body}")
        rom, pairs = build_image(fn, body, 8)
        bad = 0
        for a, v in itertools.product((0x15, 0x47, 0x6F), range(16)):
            want = int(PREDS[pred](v, c))
            got = execute(runner, patch_args(rom, pairs, 8, a, v))
            total += 1
            if got != want:
                bad += 1
                if bad <= 4:
                    print(f"  MISMATCH {fn}(a={a:#x}, v={v}): chips={got} host={want}")
        fails += bad
    print(f"\n{total - fails}/{total} compare execution checks passed on "
          f"external/chips")
    return 1 if fails else 0


if __name__ == "__main__":
    sys.exit(main())
