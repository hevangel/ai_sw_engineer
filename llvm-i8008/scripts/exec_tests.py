#!/usr/bin/env python3
"""Execution tests for llvm-i8008 (plans/testplan.md).

Each group builds one program: a driver that calls the function under test
with every case's arguments (abi.md sec.3: B C D E / BC DE) and stores each
result at 0x3000 + 2*k, then halts. The program is linked with
asm/i8008_link.py and run on the ai_hw_engineer 8008 RTL
(system/micral_n/platform/sim/micral_rtl); the results are compared with
Python's exact arithmetic.

Groups: the rt/ helpers (multiply, divide, shifts, memcpy/memmove/memset), and
compiled IR for every ALU operation, constant form, shift and compare
predicate at i8 and i16.

Run in the ai-hw-engineer image (it has Python and the newer glibc the RTL
binary needs; llc/llvm-mc come from install/bin):
    python3 llvm-i8008/scripts/exec_tests.py [cases-per-op]
"""
import os
import random
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
BACKEND = os.path.dirname(HERE)
ROOT = os.path.dirname(BACKEND)
BIN = os.path.join(BACKEND, "install", "bin")
LINK = os.path.join(BACKEND, "asm", "i8008_link.py")
RTL = os.path.join(ROOT, "system", "micral_n", "platform", "sim", "build", "micral_rtl")
RT = [os.path.join(BACKEND, "rt", f) for f in ("arith.s", "shift.s", "mem.s")]
OUT = 0x3000
DL = 'target datalayout = "e-m:e-p:16:8-i8:8-i16:8-i32:8-i64:8-n8-a:8"\ntarget triple = "i8008"\n'

N = int(sys.argv[1]) if len(sys.argv) > 1 else 40
rng = random.Random(8008)


def run(cmd, **kw):
    r = subprocess.run(cmd, capture_output=True, text=True, **kw)
    if r.returncode not in (0,):
        raise SystemExit(f"FAILED: {' '.join(cmd)}\n{r.stdout}{r.stderr}")
    return r.stdout


def s8(v):
    v &= 0xFF
    return v - 256 if v & 0x80 else v


def s16(v):
    v &= 0xFFFF
    return v - 65536 if v & 0x8000 else v


def tdiv(a, b):
    q = abs(a) // abs(b)
    return q if (a < 0) == (b < 0) else -q


def trem(a, b):
    return a - tdiv(a, b) * b


def load_args(width, args):
    """Driver code that puts the arguments in their registers."""
    out = []
    regs8 = ["b", "c", "d", "e"]
    pairs = [("c", "b"), ("e", "d")]
    if width == 8:
        for r, v in zip(regs8, args):
            out.append(f"\tl{r}i {v & 0xFF}")
    else:
        for (lo, hi), v in zip(pairs, args):
            out.append(f"\tl{lo}i {v & 0xFF}")
            out.append(f"\tl{hi}i {(v >> 8) & 0xFF}")
    return out


def store_result(k, width):
    a = OUT + 2 * k
    code = [f"\tlhi {a >> 8}", f"\tlli {a & 0xFF}"]
    if width == 8:
        code.append("\tlmb")
    else:
        code += ["\tlmc", f"\tlhi {(a + 1) >> 8}", f"\tlli {(a + 1) & 0xFF}", "\tlmb"]
    return code


def run_group(name, cases, ll=None, extra_asm="", chunk=250):
    """cases: list of (callee, width_args, args, width_ret, expected) or
    ("raw", driver_lines, width_ret, expected). Large groups run as several
    programs of `chunk` cases."""
    if len(cases) > chunk and all(c[0] != "raw" for c in cases):
        good = 0
        for i in range(0, len(cases), chunk):
            good += run_chunk(name, cases[i:i + chunk], ll, extra_asm)
    else:
        good = run_chunk(name, cases, ll, extra_asm)
    status = "ok" if good == len(cases) else "FAIL"
    print(f"{status:4} {name}: {good}/{len(cases)}")
    return good, len(cases)


def run_chunk(name, cases, ll, extra_asm):
    with tempfile.TemporaryDirectory() as d:
        drv = [".section .text.start,\"ax\",@progbits", ".globl start", "start:"]
        for k, c in enumerate(cases):
            if c[0] == "raw":
                _, lines, wret, _exp = c
                drv += lines
            else:
                callee, warg, args, wret, _exp = c
                drv += load_args(warg, args)
                drv.append(f"\tcal {callee}")
            drv += store_result(k, wret)
        drv.append("\thlt")
        drv.append(extra_asm)
        open(os.path.join(d, "drv.s"), "w").write("\n".join(drv) + "\n")
        objs = []
        for i, s in enumerate([os.path.join(d, "drv.s")] + RT):
            o = os.path.join(d, f"a{i}.o")
            run([os.path.join(BIN, "llvm-mc"), "-triple", "i8008", "-filetype=obj", s, "-o", o])
            objs.append(o)
        if ll:
            open(os.path.join(d, "f.ll"), "w").write(DL + ll)
            o = os.path.join(d, "f.o")
            run([os.path.join(BIN, "llc"), "-mtriple=i8008", "-O2", "-filetype=obj",
                 "-function-sections", os.path.join(d, "f.ll"), "-o", o])
            objs.append(o)
        lay = os.path.join(d, "lay.ld")
        open(lay, "w").write("CODE 0x0000 0x2800\nDATA follow 0x0400\nENTRY start\n")
        img = os.path.join(d, "img.bin")
        run([sys.executable, LINK, *objs, "-T", lay, "-o", img])
        dump = os.path.join(d, "out.bin")
        run([RTL, "--ram", f"{img}@0", "--max-cycles", "200000000",
             "--dump", f"{dump}@{OUT}:{2 * len(cases)}"])
        got = open(dump, "rb").read()
    bad = 0
    for k, c in enumerate(cases):
        wret, exp = (c[2], c[3]) if c[0] == "raw" else (c[3], c[4])
        v = got[2 * k] if wret == 8 else got[2 * k] | got[2 * k + 1] << 8
        e = exp & (0xFF if wret == 8 else 0xFFFF)
        if v != e:
            bad += 1
            if bad <= 8:
                desc = c[1] if c[0] == "raw" else f"{c[0]}{tuple(c[2])}"
                print(f"  MISMATCH {name}: {desc if c[0] != 'raw' else '(driver)'} = 0x{v:x}, expected 0x{e:x}")
    return len(cases) - bad


def edge8():
    return [0, 1, 2, 0x7F, 0x80, 0x81, 0xFE, 0xFF]


def edge16():
    return [0, 1, 2, 0xFF, 0x100, 0x7FFF, 0x8000, 0x8001, 0xFFFE, 0xFFFF]


def vals(width, n):
    e = edge8() if width == 8 else edge16()
    m = (1 << width) - 1
    return e + [rng.randint(0, m) for _ in range(max(0, n - len(e)))]


def pairs(width, n, nonzero_b=False, signed_ok=None):
    out = []
    va, vb = vals(width, n), vals(width, n)
    rng.shuffle(vb)
    for a, b in zip(va, vb):
        if nonzero_b and b == 0:
            b = 1
        if signed_ok and not signed_ok(a, b):
            continue
        out.append((a, b))
    return out


SEMANTICS = {
    "add": lambda a, b, w: a + b,
    "sub": lambda a, b, w: a - b,
    "and": lambda a, b, w: a & b,
    "or": lambda a, b, w: a | b,
    "xor": lambda a, b, w: a ^ b,
    "mul": lambda a, b, w: a * b,
    "udiv": lambda a, b, w: a // b,
    "urem": lambda a, b, w: a % b,
    "sdiv": lambda a, b, w: tdiv(sx(a, w), sx(b, w)),
    "srem": lambda a, b, w: trem(sx(a, w), sx(b, w)),
    "shl": lambda a, b, w: a << b,
    "lshr": lambda a, b, w: a >> b,
    "ashr": lambda a, b, w: sx(a, w) >> b,
}


def sx(v, w):
    return s8(v) if w == 8 else s16(v)


def no_overflow(w):
    lo = -(1 << (w - 1))
    return lambda a, b: not (sx(a, w) == lo and sx(b, w) == -1)


def main():
    if not os.path.exists(RTL):
        raise SystemExit(f"{RTL} missing: run system/micral_n/platform/sim/build_rtl.sh")
    total = good = 0

    def tally(r):
        nonlocal total, good
        good += r[0]
        total += r[1]

    # --- rt helpers, called directly --------------------------------------
    helpers = [
        ("__mulqi3", 8, "mul"), ("__mulhi3", 16, "mul"),
        ("__udivqi3", 8, "udiv"), ("__umodqi3", 8, "urem"),
        ("__divqi3", 8, "sdiv"), ("__modqi3", 8, "srem"),
        ("__udivhi3", 16, "udiv"), ("__umodhi3", 16, "urem"),
        ("__divhi3", 16, "sdiv"), ("__modhi3", 16, "srem"),
    ]
    for fn, w, op in helpers:
        div = op in ("udiv", "urem", "sdiv", "srem")
        ps = pairs(w, N, nonzero_b=div, signed_ok=no_overflow(w) if op in ("sdiv", "srem") else None)
        tally(run_group(fn, [(fn, w, [a, b], w, SEMANTICS[op](a, b, w)) for a, b in ps]))
    for fn, w, op in [("__ashlqi3", 8, "shl"), ("__lshrqi3", 8, "lshr"), ("__ashrqi3", 8, "ashr"),
                      ("__ashlhi3", 16, "shl"), ("__lshrhi3", 16, "lshr"), ("__ashrhi3", 16, "ashr")]:
        cases = []
        for a in vals(w, N):
            k = rng.randint(0, w - 1)
            if w == 8:
                lines = [f"\tlbi {a}", f"\tlci {k}", f"\tcal {fn}"]
            else:
                lines = [f"\tlci {a & 0xFF}", f"\tlbi {a >> 8}", f"\tldi {k}", f"\tcal {fn}"]
            cases.append(("raw", lines, w, SEMANTICS[op](a, k, w)))
        tally(run_group(fn, cases))

    # memcpy / memmove / memset on a 300-byte buffer at 0x2000 (crosses a page).
    src = [rng.randint(0, 255) for _ in range(300)]
    data = ".section .data.buf,\"aw\",@progbits\nbuf:\n" + "".join(f"\t.byte {b}\n" for b in src)
    cases, model = [], list(src)

    def call3(fn, dst, a2, n):
        lines = [f"\tlhi %hi(__args_{fn})", f"\tlli %lo(__args_{fn})", f"\tlmi {n & 0xFF}",
                 "\tinl", f"\tlmi {n >> 8}",
                 f"\tlci %lo(buf+{dst})", f"\tlbi %hi(buf+{dst})"]
        if fn == "memset":
            # c is an i8 argument after the pointer: D, as llc passes it
            # (tests/codegen/mem.ll); E is junk the routine must not read.
            lines += [f"\tldi {a2}", "\tlei 0xa5"]
        else:
            lines += [f"\tlei %lo(buf+{a2})", f"\tldi %hi(buf+{a2})"]
        return lines + [f"\tcal {fn}"]

    ops = [("memcpy", 10, 200, 60), ("memmove", 50, 40, 100), ("memmove", 0, 30, 120),
           ("memset", 250, 0x5A, 20), ("memcpy", 0, 0, 0), ("memmove", 5, 5, 3)]
    pre = []
    for fn, dst, a2, n in ops:
        pre += call3(fn, dst, a2, n)
        if fn == "memset":
            model[dst:dst + n] = [a2] * n
        else:
            model[dst:dst + n] = model[a2:a2 + n]
    for i, idx in enumerate(range(0, 300, 7)):
        lines = (pre if i == 0 else []) + [f"\tlhi %hi(buf+{idx})", f"\tlli %lo(buf+{idx})", "\tlbm"]
        cases.append(("raw", lines, 8, model[idx]))
    tally(run_group("memcpy/memmove/memset", cases, extra_asm=data))

    # --- compiled IR ------------------------------------------------------
    for w in (8, 16):
        t = f"i{w}"
        ll, cases = [], []
        for op in ["add", "sub", "and", "or", "xor", "mul", "udiv", "urem", "sdiv", "srem",
                   "shl", "lshr", "ashr"]:
            fname = f"{op}{w}"
            ll.append(f"define {t} @{fname}({t} %a, {t} %b) {{\n  %r = {op} {t} %a, %b\n  ret {t} %r\n}}")
            div = op in ("udiv", "urem", "sdiv", "srem")
            for a, b in pairs(w, N // 2, nonzero_b=div,
                              signed_ok=no_overflow(w) if op in ("sdiv", "srem") else None):
                if op in ("shl", "lshr", "ashr"):
                    b %= w
                cases.append((fname, w, [a, b], w, SEMANTICS[op](a, b, w)))
            # Constant forms.
            for k in ([1, 2, 3, w - 1] + ([7, 8, 9, 12] if w == 16 else []) if op in ("shl", "lshr", "ashr")
                      else [0, 1, 2, 0x7F, 0x80, (1 << w) - 1, (1 << w) - 2, rng.randint(1, (1 << w) - 1)]):
                if div and k == 0:
                    continue
                if op in ("sdiv", "srem") and sx(k, w) == -1:
                    continue
                fk = f"{op}{w}k{k}"
                ll.append(f"define {t} @{fk}({t} %a) {{\n  %r = {op} {t} %a, {k}\n  ret {t} %r\n}}")
                for a in vals(w, 6):
                    cases.append((fk, w, [a], w, SEMANTICS[op](a, k, w)))
        tally(run_group(f"ir alu i{w}", cases, "\n".join(ll)))

        # Compares: every predicate, register and constant forms.
        ll, cases = [], []
        preds = ["eq", "ne", "ult", "ule", "ugt", "uge", "slt", "sle", "sgt", "sge"]
        rel = {
            "eq": lambda a, b: a == b, "ne": lambda a, b: a != b,
            "ult": lambda a, b: a < b, "ule": lambda a, b: a <= b,
            "ugt": lambda a, b: a > b, "uge": lambda a, b: a >= b,
            "slt": lambda a, b: sx(a, w) < sx(b, w), "sle": lambda a, b: sx(a, w) <= sx(b, w),
            "sgt": lambda a, b: sx(a, w) > sx(b, w), "sge": lambda a, b: sx(a, w) >= sx(b, w),
        }
        for p in preds:
            fn = f"cmp{w}{p}"
            ll.append(f"define i8 @{fn}({t} %a, {t} %b) {{\n  %c = icmp {p} {t} %a, %b\n"
                      f"  %r = select i1 %c, i8 17, i8 99\n  ret i8 %r\n}}")
            ps = pairs(w, N // 2) + [(a, a) for a in vals(w, 4)]
            for a, b in ps:
                cases.append((fn, w, [a, b], 8, 17 if rel[p](a, b) else 99))
            for k in [0, 1, 0x7F, 0x80, (1 << w) - 1] + ([0x7FFF, 0x8000, 0x100] if w == 16 else []):
                fk = f"cmp{w}{p}k{k}"
                ll.append(f"define i8 @{fk}({t} %a) {{\n  %c = icmp {p} {t} %a, {k}\n"
                          f"  br i1 %c, label %y, label %n\ny:\n  ret i8 17\nn:\n  ret i8 99\n}}")
                for a in vals(w, 6) + [k, (k + 1) % (1 << w), (k - 1) % (1 << w)]:
                    cases.append((fk, w, [a], 8, 17 if rel[p](a, k) else 99))
        tally(run_group(f"ir icmp i{w}", cases, "\n".join(ll)))

    print(f"exec_tests: {good}/{total} cases passed")
    return 0 if good == total else 1


if __name__ == "__main__":
    sys.exit(main())
