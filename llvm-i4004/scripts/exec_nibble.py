#!/usr/bin/env python3
"""Execution test for the native nibble lowering (LOGICI/SHIFTI/NIBSEL).

Each case is a tiny IR function compiled by our llc, run on external/chips
behind exec_cmp.py's driver, and compared with the host computing the same
expression. Covered:

  * and/or/xor with constant masks (i8, i16): whole-nibble masks (copy /
    constant / CMA), single-bit masks (AND 1 / AND 8 short forms), and mixed
    masks that go through the ACC+CY rotate ring;
  * shl/lshr/ashr by every constant amount (i8, i16). Amounts whose inline
    expansion would be bigger than the libcall site still lower to a libcall;
    those are counted and skipped (the driver links no runtime);
  * or/xor of operands with disjoint known-zero nibbles (NIBSEL), including the
    firmware's set_digit shape `(b & 0xF0) | v` and the pack `(x << 4) | y`.

Every ALU-op case must compile WITHOUT a libcall; a `jms __and*` etc. here is a
failure (the lowering regressed to the bit-serial helpers).

Usage: exec_nibble.py        (exit status nonzero on any mismatch)
"""
import itertools
import re
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from exec_cmp import build_image, build_runner, compile_fn, execute, patch_args  # noqa: E402

VALUES = {
    8: [0x00, 0x01, 0x0F, 0x10, 0x5A, 0x7F, 0x80, 0x81, 0xA5, 0xF0, 0xFE, 0xFF],
    16: [0x0000, 0x0001, 0x00FF, 0x0F0F, 0x1234, 0x7FFF, 0x8000, 0x8001,
         0xA5C3, 0xF0F0, 0xFFFE, 0xFFFF],
}
MASKS = {
    8: [0x00, 0x01, 0x08, 0x0F, 0xF0, 0xFE, 0x7F, 0x80, 0x30, 0x5A, 0xFF],
    16: [0x0001, 0x0007, 0x00FF, 0xFF00, 0x0F0F, 0x8000, 0x7FFF, 0x1234, 0xFFFF],
}
LOGIC = {"and": lambda a, b: a & b, "or": lambda a, b: a | b,
         "xor": lambda a, b: a ^ b}


def sx(v, w):
    return v - (1 << w) if v & (1 << (w - 1)) else v


def cases():
    """Yield (name, width, ir, host(x, y) -> result, must_inline)."""
    for w in (8, 16):
        t, m = f"i{w}", (1 << w) - 1
        for (op, f), c in itertools.product(LOGIC.items(), MASKS[w]):
            name = f"{op}{w}_{c:x}"
            ir = (f"define {t} @{name}({t} %x, {t} %y) {{\n"
                  f"  %r = {op} {t} %x, {c}\n  ret {t} %r\n}}\n")
            yield name, w, ir, (lambda x, y, f=f, c=c: f(x, c)), True
        for sh, amt in itertools.product(("shl", "lshr", "ashr"), range(1, w)):
            name = f"{sh}{w}_{amt}"
            ir = (f"define {t} @{name}({t} %x, {t} %y) {{\n"
                  f"  %r = {sh} {t} %x, {amt}\n  ret {t} %r\n}}\n")
            host = {"shl": lambda x, a=amt, m=m: (x << a) & m,
                    "lshr": lambda x, a=amt: x >> a,
                    "ashr": lambda x, a=amt, w=w, m=m: (sx(x, w) >> a) & m}[sh]
            yield name, w, ir, (lambda x, y, h=host: h(x)), False
    # Add an immediate (ADDi8ri / ADDi16ri: LDM c; ADD s per nibble, IAC for
    # +1, leading zero nibbles copied); `sub x, c` arrives as `add x, -c`.
    for w, cs in ((8, [1, 2, 0x0F, 0x10, 0x31, 0x7F, 0x80, 0xFF]),
                  (16, [1, 0x10, 0xFF, 0x100, 0xFFF, 0x1234, 0x8000, 0xFFFF])):
        t, m = f"i{w}", (1 << w) - 1
        for c in cs:
            name = f"addi{w}_{c:x}"
            ir = (f"define {t} @{name}({t} %x, {t} %y) {{\n"
                  f"  %r = add {t} %x, {c}\n  ret {t} %r\n}}\n")
            yield name, w, ir, (lambda x, y, c=c, m=m: (x + c) & m), True
    # NIBSEL shapes.
    yield ("setdig8", 8,
           "define i8 @setdig8(i8 %x, i8 %y) {\n  %a = and i8 %x, -16\n"
           "  %b = and i8 %y, 15\n  %r = or i8 %a, %b\n  ret i8 %r\n}\n",
           lambda x, y: (x & 0xF0) | (y & 0x0F), True)
    yield ("pack8", 8,
           "define i8 @pack8(i8 %x, i8 %y) {\n  %a = shl i8 %x, 4\n"
           "  %b = and i8 %y, 15\n  %r = or i8 %a, %b\n  ret i8 %r\n}\n",
           lambda x, y: ((x << 4) & 0xFF) | (y & 0x0F), True)
    yield ("swap8", 8,
           "define i8 @swap8(i8 %x, i8 %y) {\n  %a = shl i8 %x, 4\n"
           "  %b = lshr i8 %x, 4\n  %r = xor i8 %a, %b\n  ret i8 %r\n}\n",
           lambda x, y: ((x << 4) & 0xFF) | (x >> 4), True)
    # Multiply by (2^k +- 1) * 2^m, k, m <= 2 -> shifts and an add (no __mulqi3).
    for w, c in ((8, 3), (8, 5), (8, 6), (8, 253), (16, 3), (16, 65533)):
        t, m = f"i{w}", (1 << w) - 1
        yield (f"mul{w}_{c}", w,
               f"define {t} @mul{w}_{c}({t} %x, {t} %y) {{\n"
               f"  %r = mul {t} %x, {c}\n  ret {t} %r\n}}\n",
               (lambda x, y, c=c, m=m: (x * c) & m), True)
    # Disjoint OR of two variables in one nibble -> ADD (no __orqi3).
    yield ("ordisj8", 8,
           "define i8 @ordisj8(i8 %x, i8 %y) {\n  %a = and i8 %x, 12\n"
           "  %b = and i8 %y, 1\n  %r = or i8 %a, %b\n  ret i8 %r\n}\n",
           lambda x, y: (x & 12) | (y & 1), True)
    yield ("ordisj16", 16,
           "define i16 @ordisj16(i16 %x, i16 %y) {\n  %a = and i16 %x, -4\n"
           "  %b = and i16 %y, 3\n  %r = or i16 %a, %b\n  ret i16 %r\n}\n",
           lambda x, y: (x & 0xFFFC) | (y & 3), True)
    yield ("bytes16", 16,
           "define i16 @bytes16(i16 %x, i16 %y) {\n  %a = and i16 %x, -256\n"
           "  %b = lshr i16 %y, 8\n  %r = or i16 %a, %b\n  ret i16 %r\n}\n",
           lambda x, y: (x & 0xFF00) | (y >> 8), True)


def main():
    runner = build_runner()
    fails = total = libcalls = 0
    for name, w, ir, host, must_inline in cases():
        body = compile_fn(ir)
        if re.search(r"jms\s+__", body):
            if must_inline:
                print(f"{name:14s} FAIL: lowered to a libcall\n{body}")
                fails += 1
            else:
                libcalls += 1
            continue
        rom, pairs = build_image(name, body, w, ret_width=w)
        key = "result" if w == 8 else "result16"
        bad = 0
        vals = VALUES[w]
        for x, y in itertools.product(vals, vals[:4] if not name.startswith(
                ("setdig", "pack", "swap", "bytes", "ordisj", "mul")) else vals):
            want = host(x, y)
            got = execute(runner, patch_args(rom, pairs, w, x, y), key)
            total += 1
            if got != want:
                bad += 1
                if bad <= 3:
                    print(f"  MISMATCH {name}(0x{x:x},0x{y:x}): "
                          f"chips=0x{got:x} host=0x{want:x}")
        fails += bad
        print(f"{name:14s} {len(rom):4d} B  {'OK' if not bad else f'{bad} bad'}")
    print(f"\n{total - fails}/{total} nibble-op execution checks passed on "
          f"external/chips ({libcalls} constant shifts left to libcalls)")
    return 1 if fails else 0


if __name__ == "__main__":
    sys.exit(main())
