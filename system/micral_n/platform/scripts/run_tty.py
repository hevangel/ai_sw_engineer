#!/usr/bin/env python3
r"""Replay teletype transcripts against a Micral N program on a simulator.

    run_tty.py --ram prog.bin [--boot boot.bin] [transcript.txt ...]
    run_tty.py --disk disk.img --boot boot.bin [transcript.txt ...]

Each transcript holds `<` lines (keys typed) and `>` lines (bytes printed),
with \r \n \\ \xNN escapes (the titles' oracle format). The program starts
at address 0: loaded directly with --ram, or from a disk image by a boot ROM
(--disk + --boot, the machine's real path). The run ends when the program
halts or has waited for keys with none left; the printed bytes must equal
the `>` lines exactly.

Simulator: MICRAL_SIM, default the RTL harness platform/sim/build/micral_rtl.
"""
import argparse
import os
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
SIM = os.environ.get("MICRAL_SIM", os.path.join(HERE, "..", "sim", "build", "micral_rtl"))


def unescape(s):
    out, i = bytearray(), 0
    while i < len(s):
        if s[i] == "\\" and i + 1 < len(s):
            c = s[i + 1]
            if c == "x":
                out.append(int(s[i + 2:i + 4], 16))
                i += 4
                continue
            out.append({"r": 13, "n": 10, "\\": 92}[c])
            i += 2
        else:
            out.append(ord(s[i]))
            i += 1
    return bytes(out)


def load(path):
    keys, want = bytearray(), bytearray()
    for line in open(path, encoding="latin-1"):
        line = line.rstrip("\n")
        if line.startswith("< "):
            keys += unescape(line[2:])
        elif line.startswith("> "):
            want += unescape(line[2:])
    return bytes(keys), bytes(want)


def run(args, keys):
    with tempfile.TemporaryDirectory() as d:
        tin, tout = os.path.join(d, "in"), os.path.join(d, "out")
        open(tin, "wb").write(keys)
        cmd = [SIM, "--tty-in", tin, "--tty-out", tout, "--idle", "20000",
               "--max-cycles", str(args.max_cycles), "--regs"]
        if args.ram:
            cmd += ["--ram", f"{args.ram}@0"]
        if args.boot:
            cmd += ["--boot", args.boot]
        if args.disk:
            cmd += ["--disk", args.disk]
        r = subprocess.run(cmd, capture_output=True, text=True)
        return open(tout, "rb").read(), r


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--ram")
    ap.add_argument("--boot")
    ap.add_argument("--disk")
    ap.add_argument("--max-cycles", type=int, default=2_000_000_000)
    ap.add_argument("transcripts", nargs="+")
    args = ap.parse_args()
    bad = 0
    for p in args.transcripts:
        keys, want = load(p)
        got, r = run(args, keys)
        ok = got == want and r.returncode == 0
        print(("ok  " if ok else "FAIL"), os.path.basename(p), r.stderr.strip().split("micral_rtl: ")[-1])
        if not ok:
            bad += 1
            at = next((i for i, (a, b) in enumerate(zip(got, want)) if a != b), min(len(got), len(want)))
            print(f"    first difference at byte {at} (got {len(got)}, want {len(want)})")
            print(f"    got:  {got[max(0, at - 30):at + 30]!r}")
            print(f"    want: {want[max(0, at - 30):at + 30]!r}")
    print(f"run_tty: {len(args.transcripts) - bad}/{len(args.transcripts)} transcripts match")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
