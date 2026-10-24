#!/usr/bin/env python3
"""Replay the monitor transcripts on a Micral N simulator (testplan.md).

    run_target.py <monitor.bin> <monitor.map> [transcript.txt ...]

For each transcript (default: every oracle/transcripts/*.txt):
  * `=` lines preload RAM, `R` lines preload the saved registers (mon_regs);
  * the console starts the monitor with `JMP 3B30` (cold), or `JMP 3B49`
    (warm) when the transcript preloads registers, since the cold entry
    zeroes them;
  * `<` lines are typed; the run stops when the monitor has waited for input
    long enough with nothing left to type, or the CPU halts (a `G` into
    empty memory);
  * the printed text must equal the `>` lines, and `?` lines are checked in
    a memory dump. The host mock's hold entries 0x3800 / 0x3808 stand for
    the real mon_hold0 / mon_hold1 from the map.

Transcripts whose name starts with `h` are host-only (the mock's `G` returns
to the command loop, a real one does not); `t` transcripts run only here.
"""
import os
import re
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
TITLE = os.path.dirname(HERE)
ROOT = os.path.abspath(os.path.join(TITLE, "..", "..", ".."))
RTL = os.environ.get("MICRAL_SIM", os.path.join(ROOT, "system", "micral_n", "platform", "sim",
                                                "build", "micral_rtl"))


def unescape(s):
    out, i = bytearray(), 0
    while i < len(s):
        if s[i] == "\\" and i + 1 < len(s):
            c = s[i + 1]
            if c == "r":
                out.append(13)
            elif c == "n":
                out.append(10)
            elif c == "\\":
                out.append(92)
            elif c == "x":
                out.append(int(s[i + 2:i + 4], 16))
                i += 2
            i += 2
        else:
            out.append(ord(s[i]))
            i += 1
    return bytes(out)


def symbols(mapfile):
    syms = {}
    for line in open(mapfile):
        m = re.match(r"image 0x([0-9a-f]{4})", line)
        if m:
            syms["__image"] = int(m.group(1), 16)
        m = re.match(r"\s+0x([0-9a-f]{4}) (\S+)", line)
        if m:
            syms[m.group(2)] = int(m.group(1), 16)
    return syms


def run_one(image, syms, path):
    inp, want, pre, checks, regs = bytearray(), bytearray(), [], [], None
    for line in open(path):
        line = line.rstrip("\n")
        if line.startswith("< "):
            inp += unescape(line[2:])
        elif line.startswith("> "):
            want += unescape(line[2:])
        elif line.startswith("= "):
            f = line[2:].split()
            pre.append((int(f[0], 16), bytes(int(x, 16) for x in f[1:])))
        elif line.startswith("? "):
            f = line[2:].split()
            checks.append((int(f[0], 16), [int(x, 16) for x in f[1:]]))
        elif line.startswith("R "):
            regs = bytes(int(x, 16) for x in line[2:].split())
    hold = {0x3800: syms["mon_hold0"], 0x3808: syms["mon_hold1"]}
    with tempfile.TemporaryDirectory() as d:
        args = [RTL, "--rom", f"{image}@{syms['__image']}", "--start", "0x3b49" if regs else "0x3b30",
                "--idle", "3000", "--max-cycles", "50000000"]
        if regs:
            pre.append((syms["mon_regs"], regs))
        for k, (a, data) in enumerate(pre):
            f = os.path.join(d, f"pre{k}.bin")
            open(f, "wb").write(data)
            args += ["--ram", f"{f}@{a}"]
        tin = os.path.join(d, "in.txt")
        open(tin, "wb").write(bytes(inp))
        out = os.path.join(d, "out.txt")
        dump = os.path.join(d, "mem.bin")
        args += ["--tty-in", tin, "--tty-out", out, "--dump", f"{dump}@0:16384"]
        r = subprocess.run(args, capture_output=True, text=True)
        got = open(out, "rb").read()
        mem = open(dump, "rb").read()
    errs = []
    if r.returncode not in (0,):
        errs.append(f"simulator exit {r.returncode}: {r.stderr.strip()}")
    if got != bytes(want):
        errs.append(f"output differs\n  got:  {got!r}\n  want: {bytes(want)!r}")
    for a, vals in checks:
        if len(vals) == 3 and vals[0] == 0x46:   # a hold: CAL <entry>
            e = hold.get(vals[1] | vals[2] << 8)
            if e is not None:
                vals = [0x46, e & 0xFF, e >> 8]
        if list(mem[a:a + len(vals)]) != vals:
            errs.append(f"memory at {a:04X}: {list(mem[a:a + len(vals)])}, want {vals}")
    return errs


def main():
    image, mapfile = sys.argv[1], sys.argv[2]
    names = sys.argv[3:] or sorted(
        os.path.join(TITLE, "oracle", "transcripts", n)
        for n in os.listdir(os.path.join(TITLE, "oracle", "transcripts"))
        if n.endswith(".txt") and not n.startswith("h"))
    syms = symbols(mapfile)
    bad = 0
    for p in names:
        errs = run_one(image, syms, p)
        print(("ok  " if not errs else "FAIL"), os.path.basename(p))
        for e in errs:
            print("    " + e)
        bad += bool(errs)
    print(f"run_target: {len(names) - bad}/{len(names)} transcripts match")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
