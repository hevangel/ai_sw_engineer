#!/usr/bin/env python3
"""Execution test of the BCD primitives (asm/num.s) on external/chips.

Each leaf of `src/bcd.rs` `prim` exists twice: the MCS-4 routine in
asm/num.s (what the 4004 runs) and its Rust twin (what the host tests run).
This test proves them identical: for every leaf and many random operand RAM
images it

  * links a driver (`fim p1, a1; fim p2, a2; jms <leaf>`; store the result
    nibbles at 0xF0/0xF1; mark done) with num.s through the image packer,
  * runs it on external/chips with CM-RAM bank 0 preloaded (tools/
    chips_xcheck run_ram),
  * runs the Rust twin on the same image (examples/num_twin.rs),

and requires the whole bank-0 RAM image -- 256 main and 64 status
characters -- and the result to match.

Usage: exec_num.py [cases-per-leaf]      (run inside the ai-sw-engineer
container; exit status nonzero on any mismatch)
"""
import json
import os
import random
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
TITLE = os.path.dirname(HERE)
ROOT = os.path.abspath(os.path.join(TITLE, "..", "..", ".."))
BACKEND = os.path.join(ROOT, "llvm-i4004")
BIN = os.path.join(BACKEND, "install", "bin")
LLVM_MC = os.environ.get("LLVM_MC", os.path.join(BIN, "llvm-mc"))
PACK = os.path.join(BACKEND, "asm", "i4004_pack.py")
XCHECK = os.path.join(BACKEND, "tools", "chips_xcheck")
NUM_S = os.path.join(TITLE, "asm", "num.s")

DRIVER = """\
	.section .reset,"ax"
	.globl	_start
_start:
	fim	p1, 0
	fim	p2, 0
	fim	p3, 0
	fim	p4, 0x5A	; canaries: a __leaf_* routine must preserve P4-P6,
	fim	p5, 0xC3	; every routine P6
	fim	p6, 0x96
	jms	{leaf}
	fim	p0, 0xF0
	src	p0
	ld	r3
	wrm
	inc	r1
	src	p0
	ld	r2
	wrm
	inc	r1
	src	p0
	ld	r8
	wrm
	inc	r1
	src	p0
	ld	r9
	wrm
	inc	r1
	src	p0
	ld	r10
	wrm
	inc	r1
	src	p0
	ld	r11
	wrm
	inc	r1
	src	p0
	ld	r12
	wrm
	inc	r1
	src	p0
	ld	r13
	wrm
	ldm	10
	wr3
1:	jun	1b
"""
CANARY = [0x5, 0xA, 0xC, 0x3, 0x9, 0x6]  # R8..R13


def run(cmd, **kw):
    return subprocess.run(cmd, check=True, capture_output=True, text=True, **kw)


def target_dir(cwd):
    meta = run(["cargo", "metadata", "--format-version", "1", "--no-deps"], cwd=cwd)
    return json.loads(meta.stdout)["target_directory"]


def build_image(leaf, work):
    s = os.path.join(work, f"{leaf}.s")
    sym = leaf if leaf in HEAVY else "__leaf_" + leaf[2:]
    with open(s, "w") as f:
        f.write(DRIVER.format(leaf=sym))
    drv, num = os.path.join(work, f"{leaf}.o"), os.path.join(work, "num.o")
    run([LLVM_MC, "-triple=i4004", "-filetype=obj", s, "-o", drv])
    if not os.path.exists(num):
        run([LLVM_MC, "-triple=i4004", "-filetype=obj", NUM_S, "-o", num])
    out = os.path.join(work, f"{leaf}.bin")
    run(["python3", PACK, drv, num, "-o", out, "--gc-sections", "--max", "4096"])
    with open(out, "rb") as f:
        rom = bytearray(f.read())
    assert rom[0] == 0x22 and rom[2] == 0x24 and rom[4] == 0x26, "driver FIM layout changed"
    return rom


# --- RAM images -------------------------------------------------------------
def blank():
    return [0] * 320


def put_num(img, reg, digits, rng, status=True):
    """digits: list, least significant first (len <= 16)."""
    for i in range(16):
        img[reg + i] = digits[i] if i < len(digits) else 0
    if status:
        r = reg >> 4
        for k in range(4):
            img[256 + r * 4 + k] = rng.randrange(16)


def put_operand(img, reg, digits, rng, max_scale):
    """A calculator number: sign 0/1, scale byte 0..max_scale-1."""
    put_num(img, reg, digits, rng)
    r, s = reg >> 4, rng.randrange(max_scale)
    img[256 + r * 4 + 0] = rng.randrange(2)
    img[256 + r * 4 + 1] = s & 15
    img[256 + r * 4 + 2] = s >> 4


def rand_digits(rng, n=None):
    n = rng.randrange(17) if n is None else n
    d = [rng.randrange(10) for _ in range(n)]
    if n and rng.randrange(4):
        d[-1] = 1 + rng.randrange(9)
    return d


def noise(rng):
    """Random BCD digits everywhere and random status characters."""
    img = [rng.randrange(10) for _ in range(256)] + [rng.randrange(16) for _ in range(64)]
    img[256 + 15 * 4 + 3] = 0  # the done marker
    return img


HANDLES = [0x00, 0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70]


def cases(leaf, rng, n):
    for _ in range(n):
        img = noise(rng)
        a = rng.choice(HANDLES)
        b = rng.choice([h for h in HANDLES if h != a])
        if leaf in ("__n_clear", "__n_digits", "__n_inc"):
            if leaf == "__n_digits":
                put_num(img, a, rand_digits(rng), rng)
            if leaf == "__n_inc" and rng.randrange(3) == 0:
                put_num(img, a, [9] * rng.randrange(17), rng)
            yield a, 0, img
        elif leaf in ("__n_copy", "__n_cpyd"):
            yield a, b, img
        elif leaf == "__n_cp":
            yield a + (b >> 4), 0, img
        elif leaf in ("__n_cmp", "__n_add16", "__n_sub16"):
            da = rand_digits(rng)
            db = list(da) if rng.randrange(3) == 0 else rand_digits(rng)
            if db and rng.randrange(2):
                db[rng.randrange(len(db))] = rng.randrange(10)
            put_num(img, a, da, rng)
            put_num(img, b, db, rng)
            yield a, b, img
        elif leaf in ("__n_shl", "__n_shr"):
            put_num(img, a, rand_digits(rng), rng)
            k = rng.choice([0, 1, 2, 3, 5, 9, 14, 15, 16, 17, 18, 26, 31, 32, 47])
            yield a, k, img
        elif leaf in ("__b_clear", "__b_up"):
            buf = rng.choice([0x80, 0x8E, 0x94, 0xA2, 0xB0])
            ln = rng.choice([1, 15, 16, 17, 20, 32, 34, 40])
            ln = min(ln, 0xE0 - buf)
            yield buf, (-ln) & 0xFF, img
        elif leaf == "__b_addsh":
            # A partial product: the 32-digit buffer at 0x80 below 10^31 and
            # the addend shifted by i, so the carry stays inside it.
            i = rng.randrange(16)
            for k in range(32):
                img[0x80 + k] = rng.randrange(10) if k < 31 else 0
            put_num(img, b, rand_digits(rng, 16 - i) if i else rand_digits(rng, 15), rng)
            yield 0x80 + i, b, img
        elif leaf == "__b_divdig":
            div = rng.choice(HANDLES)
            dd = 1 + rng.randrange(16)
            put_num(img, div, rand_digits(rng, dd)[:dd - 1] + [1 + rng.randrange(9)], rng)
            # the remainder (17 digits at 0x9F) is below 10 * div, as in __n_div
            d = sum(img[div + k] * 10 ** k for k in range(16))
            r = rng.randrange(10) * d + rng.randrange(d)
            for k in range(17):
                img[0x9F + k] = (r // 10 ** k) % 10
            yield div, 0, img
        elif leaf == "__n_div":
            # dst may alias an operand; operands with up to 14 digits, scales
            # 0..13, either sign; FL (a|1) or a fixed frac 0..14 (dst|frac).
            dst = rng.choice(HANDLES)
            for h in (a, b):
                nd = rng.choice([0, 1, 2, 5, 9, 14, rng.randrange(15)])
                put_num(img, h, rand_digits(rng, nd), rng)
                r = h >> 4
                img[256 + r * 4 + 0] = rng.randrange(2)
                img[256 + r * 4 + 1] = rng.randrange(14)
                img[256 + r * 4 + 2] = 0
            if rng.randrange(3):
                yield dst | rng.randrange(15), a, b, img
            else:
                yield dst, a | 1, b, img
        elif leaf == "__n_width":
            put_num(img, a, rand_digits(rng), rng)
            r = a >> 4
            img[256 + r * 4 + 1] = rng.randrange(16)
            img[256 + r * 4 + 2] = rng.choice([0, 0, 0, 1, 15])
            yield a, 0, img
        elif leaf in ("__n_mul", "__n_add", "__n_round", "__n_sqrt"):
            dst = rng.choice(HANDLES)
            for h in (a, b):
                nd = rng.choice([0, 1, 2, 5, 9, 14, 16, rng.randrange(17)])
                put_operand(img, h, rand_digits(rng, nd), rng,
                            27 if leaf == "__n_round" else 14)
            frac = rng.randrange(15)
            if leaf == "__n_mul":
                yield dst, a, b, img
            elif leaf == "__n_add":
                yield dst | rng.randrange(2), a, b, img
            elif leaf == "__n_round":
                # frac and the 5/4 switch come from the selectors (0xC2 S2,
                # 0xC3 S3: noise, so 5/4 = 1 shows up one time in 16)
                if rng.randrange(4) == 0:
                    dst = a
                img[256 + 0xC * 4 + 3] = rng.choice([1, 1, 0, 2, rng.randrange(16)])
                yield dst, a, 0, img
            else:
                yield dst, a, frac, img
        elif leaf == "__n_compute":
            # the pending operation (MOP = S3 of 0x40) on DR, WR, CR
            for h in (0x00, 0x10, 0x20):
                put_operand(img, h, rand_digits(rng, rng.choice([0, 1, 5, 9, 14])), rng, 14)
            img[256 + 4 * 4 + 3] = rng.randrange(5)
            yield 0, 0, img
        elif leaf == "__b_sqdig":
            r = rng.randrange(10 ** rng.randrange(1, 17))
            s = 20 * r + 1
            rem = rng.randrange(200 * r + 100)
            for k in range(20):
                img[0x80 + k] = (rem // 10 ** k) % 10
                img[0x94 + k] = (s // 10 ** k) % 10
            yield 0x80, 0x94, img
        else:
            raise ValueError(leaf)


# Whole-operation entry routines (twin: the Rust `Num` operation). They use
# their own scratch, so only the architectural registers 0x00-0x7F are compared.
ENTRIES = ["__n_div", "__n_mul", "__n_add", "__n_round", "__n_sqrt", "__n_compute"]

# The two routines that use P4:P5 keep their plain names; the rest are
# `__leaf_*` symbols on the target (they preserve P4-P6).
HEAVY = set(ENTRIES)

# Leaves without a result (P1 holds leftovers).
VOID = {"__n_clear", "__n_copy", "__n_cp", "__n_cpyd", "__n_sub16", "__b_clear", "__b_up",
        "__b_addsh", "__n_sqrt"}

LEAVES = ENTRIES + ([] if os.environ.get("ONLY") else ["__n_clear", "__n_copy", "__n_cp", "__n_cpyd", "__n_digits", "__n_width", "__n_cmp",
          "__n_add16", "__n_sub16", "__n_inc", "__n_shl", "__n_shr",
          "__b_clear", "__b_up", "__b_addsh", "__b_divdig", "__b_sqdig"])


def main():
    n = int(sys.argv[1]) if len(sys.argv) > 1 else 200
    run(["cargo", "build", "--quiet", "--release", "--bin", "run_ram"], cwd=XCHECK)
    runner = os.path.join(target_dir(XCHECK), "release", "run_ram")
    run(["cargo", "build", "--quiet", "--release", "--example", "num_twin"], cwd=TITLE)
    twin = os.path.join(target_dir(TITLE), "release", "examples", "num_twin")
    rng = random.Random(0x4004)
    total = fails = 0
    with tempfile.TemporaryDirectory() as work:
        for leaf in LEAVES:
            rom = build_image(leaf, work)
            todo = [c if len(c) == 4 else (c[0], c[1], 0, c[2])
                    for c in cases(leaf, rng, n)]
            lines = "".join(f"{leaf} {a1} {a2} {a3} {''.join('%x' % v for v in img)}\n"
                            for a1, a2, a3, img in todo)
            want = run([twin], input=lines).stdout.splitlines()
            bad = 0
            for (a1, a2, a3, img), w in zip(todo, want):
                rom[1], rom[3], rom[5] = a1, a2, a3
                rp = os.path.join(work, "case.bin")
                with open(rp, "wb") as f:
                    f.write(rom)
                cycles = "6000000" if leaf in ENTRIES else "400000"
                out = run([runner, rp, "".join("%x" % v for v in img), cycles]).stdout
                kv = dict(l.split("=", 1) for l in out.split())
                got = [int(c, 16) for c in kv["ram"]]
                if kv["done"] != "1":
                    bad += 1
                    if bad <= 3:
                        print(f"  {leaf}({a1:#x},{a2:#x}): did not finish")
                    continue
                res = got[0xF0] | got[0xF1] << 4
                regs = got[0xF2:0xF8]  # R8..R13 after the call
                want_regs = CANARY if leaf not in HEAVY else regs[:4] + CANARY[4:]
                if regs != want_regs:
                    bad += 1
                    if bad <= 3:
                        print(f"  {leaf}: clobbered R8..R13 -> {regs} (contract {want_regs})")
                    continue
                for k in range(0xF0, 0xF8):
                    got[k] = img[k]
                got[256 + 15 * 4 + 3] = 0
                wres, wimg = w.split()
                wimg = [int(c, 16) for c in wimg]
                if leaf in ENTRIES:
                    # registers 0x00-0x7F, main and status characters
                    keep = list(range(0x80)) + list(range(256, 256 + 32))
                    got = [got[i] for i in keep]
                    wimg = [wimg[i] for i in keep]
                if (leaf not in VOID and res != int(wres, 16)) or got != wimg:
                    bad += 1
                    if bad <= 3:
                        diff = [i for i in range(len(got)) if got[i] != wimg[i]]
                        print(f"  {leaf}({a1:#x},{a2:#x}): result chips={res:#x} "
                              f"twin={wres}; RAM differs at {[hex(i) for i in diff[:12]]}")
            total += len(todo)
            fails += bad
            print(f"{leaf:12s} {len(todo) - bad}/{len(todo)}")
    print(f"\n{total - fails}/{total} BCD primitive checks: asm/num.s == Rust twin "
          f"on external/chips")
    return 1 if fails else 0


if __name__ == "__main__":
    sys.exit(main())
