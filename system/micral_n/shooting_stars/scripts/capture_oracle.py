#!/usr/bin/env python3
r"""Capture Shooting Stars oracle transcripts from the ORIGINAL program.

    capture_oracle.py [original.bin]   (default build/original.bin)

Runs build/original.bin (scripts/build_original.sh: the 1976 listing with the
Micral N teletype) on the ai_hw_engineer RTL harness for each scenario below
and writes oracle/transcripts/NN_name.txt: `<` lines are the keys typed, `>`
lines the printed bytes (\r \n \xNN escapes), split after each LF. The game
halts after the final "no"; the transcript ends there.

The shot sequences: the shortest win (11 shots, "BEST POSSIBLE SCORE IS 11
SHOTS") and a shortest loss, found by breadth-first search over the game's
mask table.
"""
import hashlib
import os
import pathlib
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
TITLE = os.path.dirname(HERE)
RTL = os.environ.get("MICRAL_SIM", os.path.join(TITLE, "..", "platform", "sim", "build", "micral_rtl"))


def contained(base, *parts):
    """Resolve parts under base and refuse paths that escape it (SAST guard)."""
    real_base = pathlib.Path(base).resolve()
    p = real_base.joinpath(*parts).resolve()
    if p != real_base and real_base not in p.parents:
        raise SystemExit(f"path escapes {real_base}: {parts}")
    return p

WIN = "52138752985"
LOSE = "52468"
SCENARIOS = [
    ("01_rules_and_win", "Y\r" + "   " + WIN + "N\r"),
    ("02_no_rules_lose", "N\r" + LOSE + "N\r"),
    ("03_bad_shots", "N\r" + "1A0" + WIN + "N\r"),
    ("04_play_again", "NO\r" + LOSE + "YES\r" + WIN + "NOPE\r"),
]


def esc(b):
    out = []
    for c in b:
        if c == 13:
            out.append(r"\r")
        elif c == 10:
            out.append(r"\n")
        elif c == 92:
            out.append("\\\\")
        elif 32 <= c < 127:
            out.append(chr(c))
        else:
            out.append(r"\x%02x" % c)
    return "".join(out)


def main():
    orig = sys.argv[1] if len(sys.argv) > 1 else os.path.join(TITLE, "build", "original.bin")
    sha = hashlib.sha256(open(orig, "rb").read()).hexdigest()
    for name, keys in SCENARIOS:
        with tempfile.TemporaryDirectory() as d:
            tin, tout = contained(d, "in"), contained(d, "out")
            tin.write_bytes(keys.encode())
            r = subprocess.run([RTL, "--ram", f"{orig}@0", "--tty-in", tin, "--tty-out", tout,
                                "--idle", "20000", "--max-cycles", "200000000", "--regs"],
                               capture_output=True, text=True)
            out = tout.read_bytes()
        if "halted" not in r.stderr:
            raise SystemExit(f"{name}: the original did not halt: {r.stderr}")
        lines = [f"# Captured from the original (build/original.bin, SHA-256 {sha}) on the",
                 "# ai_hw_engineer 8008 RTL by scripts/capture_oracle.py. Do not edit.",
                 "< " + esc(keys.encode())]
        cur = bytearray()
        for c in out:
            cur.append(c)
            if c == 10:
                lines.append("> " + esc(cur))
                cur = bytearray()
        if cur:
            lines.append("> " + esc(cur))
        p = contained(TITLE, "oracle", "transcripts", name + ".txt")
        p.write_text("\n".join(lines) + "\n", newline="\n")
        print(f"{name}: {len(out)} bytes")


if __name__ == "__main__":
    main()
