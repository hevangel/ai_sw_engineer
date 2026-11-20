#!/usr/bin/env python3
"""Compare the Rust emulator against an independent simulator on every manual
example: Patrick Leboutillier's `inside4004` (Python, RTL-level MCS-4 model
with its own keyboard and printer sensors, CC BY-NC-SA 4.0).

inside4004 is run, never copied: clone it outside this repository and pass its
path. The script checks that the ROM text shipped with it is byte-identical to
the ROM given with --rom (SHA-256), replays each example's keystrokes through
it, and compares the printed tape (digits, left and right symbol columns) row
for row with the tape the Rust emulator prints for the same example. Blank
rows are ignored. Paper colour is not compared: inside4004 never clears its
red state.

inside4004 has no key for the diamond (ST) or 00 keys; they are added to its
keyboard table at run time (the table lists them as unused), without editing
its files.

    py compare_inside4004.py --inside4004 DIR --rom ROM.bin \
        --emulator emulator/target/release/busicom-emulator.exe
"""
import argparse
import concurrent.futures as cf
import hashlib
import json
import os
import pathlib
import subprocess
import sys
import tempfile

HERE = pathlib.Path(__file__).resolve().parent
ORACLE = HERE.parent.parent / "firmware" / "oracle" / "manual_examples.json"

# manual-example key name -> inside4004 keyboard token
KEYS = {"C": "CL", "SQRT": "SQ", "SIGN": "S-"}
DP_ORDER = [0, 1, 2, 3, 4, 5, 6, 8]  # one 'd' press advances along this ring
RND_ORDER = [0, 1, 8]  # one 'r' press advances along this ring

# emulator symbol -> inside4004 symbol
SYM_A = {"◇": "<>", "×": "X", "÷": "/", "√": "SQ"}
SYM_B = {"Ⅰ": "I", "Ⅱ": "II", "Ⅲ": "III"}

DRIVER = """
import runpy, sys
sys.path[:0] = [".", "chips/modules", "hdl"]
import chips.keyboard as kb
kb._lookup[2][3] = "ST"   # diamond key, listed as unused
kb._lookup[5][0] = "00"   # 00 key, listed as unused
sys.argv = ["mcs4.py", "-o", "-kb", sys.argv[1], "141-PF/ROM.bin"]
runpy.run_path("141-PF/mcs4.py", run_name="__main__")
"""


def rom_from_text(path):
    out = bytearray()
    for line in path.read_text().splitlines():
        field = line.split("#")[0].strip()
        if field:
            out.append(int(field, 2))
    return bytes(out)


def keystrokes(example):
    """Token list for one example, including switch changes."""
    tokens, dp, rnd = [], 0, 0
    sw = dict(example["switches"])

    def switch(target_dp, target_rnd):
        nonlocal dp, rnd
        n = (DP_ORDER.index(target_dp) - DP_ORDER.index(dp)) % len(DP_ORDER)
        tokens.extend(["d"] * n)
        dp = target_dp
        n = (RND_ORDER.index(target_rnd) - RND_ORDER.index(rnd)) % len(RND_ORDER)
        tokens.extend(["r"] * n)
        rnd = target_rnd

    steps = [{"keys": example["setup_keys"]}] + example["steps"]
    for step in steps:
        sw.update(step.get("switches", {}))
        switch(sw["precision"], sw["rounding"])
        tokens.extend(KEYS.get(k, k) for k in step.get("keys", []))
    return tokens + ["a"] * 8


def run_inside(root, example):
    keys = ",".join(keystrokes(example) + ["q"])
    env = dict(os.environ, PYTHONPATH=str(root))
    done = subprocess.run(
        [sys.executable, "-c", DRIVER, keys],
        cwd=root, env=env, capture_output=True, text=True, timeout=1800,
    )
    rows = []
    for line in done.stdout.splitlines():
        if line.startswith(">>>") and line.rstrip().endswith("|"):
            body = line[line.index("|") + 1 : line.rindex("|")]
            body = body.ljust(22)
            row = (body[:15].strip(), body[16:18].strip(), body[19:22].strip())
            if any(row):
                rows.append(row)
    return rows


def emulator_rows(exe, rom, doc):
    schedule = subprocess.run(
        [sys.executable, str(HERE / "build_schedule.py"), str(ORACLE)],
        capture_output=True, text=True, check=True,
    ).stdout
    with tempfile.NamedTemporaryFile("w", suffix=".sched", delete=False) as fh:
        fh.write(schedule)
        name = fh.name
    try:
        trace = subprocess.run(
            [str(exe), str(rom), name], capture_output=True, text=True,
            encoding="utf-8", check=True, timeout=3600,
        ).stdout
    finally:
        os.unlink(name)
    rows, current = {}, None
    for line in trace.splitlines():
        if line.startswith("#EXAMPLE "):
            current = line.split()[1]
            rows[current] = []
        elif line.startswith("ROW|") and current:
            _, digits, a, b, _red = line.split("|")
            rows[current].append((digits.strip(), SYM_A.get(a.strip(), a.strip()),
                                  SYM_B.get(b.strip(), b.strip())))
    return {k: [r for r in v if any(r)] for k, v in rows.items()}


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawTextHelpFormatter)
    ap.add_argument("--inside4004", required=True, type=pathlib.Path)
    ap.add_argument("--rom", required=True, type=pathlib.Path)
    ap.add_argument("--emulator", required=True, type=pathlib.Path)
    ap.add_argument("--jobs", type=int, default=4)
    ap.add_argument("--only", help="comma-separated example ids")
    args = ap.parse_args()

    ours = rom_from_text(args.inside4004 / "141-PF" / "ROM.bin")
    given = args.rom.read_bytes()
    if hashlib.sha256(ours).digest() != hashlib.sha256(given).digest():
        sys.exit("inside4004 ROM differs from --rom; refusing to compare")
    doc = json.loads(ORACLE.read_text(encoding="utf-8"))
    examples = doc["examples"]
    if args.only:
        wanted = set(args.only.split(","))
        examples = [e for e in examples if e["id"] in wanted]
    mine = emulator_rows(args.emulator, args.rom, doc)
    with cf.ThreadPoolExecutor(args.jobs) as pool:
        theirs = list(pool.map(lambda e: run_inside(args.inside4004, e), examples))
    bad = 0
    for example, rows in zip(examples, theirs):
        expect = mine[example["id"]]
        if rows == expect:
            print(f"{example['id']}: identical ({len(rows)} rows)")
        else:
            bad += 1
            print(f"{example['id']}: DIFFERENT (inside4004 {len(rows)} rows, emulator {len(expect)})")
            for i, (a, b) in enumerate(zip(rows, expect)):
                if a != b:
                    print(f"  row {i + 1}: inside4004 {a} | emulator {b}")
                    break
            else:
                longer = rows if len(rows) > len(expect) else expect
                print(f"  extra row {len(min(rows, expect, key=len)) + 1}: {longer[len(min(rows, expect, key=len))]}")
    print(f"{len(examples) - bad}/{len(examples)} examples identical")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
