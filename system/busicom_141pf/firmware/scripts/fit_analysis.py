#!/usr/bin/env python3
"""Measure the firmware's RAM and ROM footprint from the whole-program IR.

RAM footprint is what decides whether the firmware fits the stock Busicom board
(two 4002s = 64 bytes of main memory + 32 nibbles of status). We read it from
the retargeted i4004 IR, which is the actual thing the backend will lower:

  * Static state  — every module-level non-constant global, sized by its type.
  * Live frames   — the sum of `alloca` sizes per function is that function's
    worst-case local frame. The call graph is <= 3 deep by design, so the peak
    simultaneous RAM is the heaviest root-to-leaf path's frames plus the saved-
    register/return overhead the backend adds. We report the biggest single
    object (the `Calculator`) and per-function alloca totals; the backend's
    final numbers refine these, but the dominant term is visible now.

ROM is reported as the IR instruction/function count as a proxy (real byte size
needs the backend; recorded as pending).

Usage: fit_analysis.py <firmware_i4004.ll>
"""
from __future__ import annotations

import re
import sys
from collections import defaultdict


def ty_size(ty: str) -> int:
    """Byte size of a simple IR type on the i4004 datalayout (byte-granular:
    iN -> ceil(N/8), ptr -> 2 bytes, [N x T] -> N*size(T))."""
    ty = ty.strip()
    m = re.fullmatch(r"\[(\d+) x (.+)\]", ty)
    if m:
        return int(m.group(1)) * ty_size(m.group(2))
    if ty == "ptr":
        return 2
    m = re.fullmatch(r"i(\d+)", ty)
    if m:
        return (int(m.group(1)) + 7) // 8
    return 0


def main() -> int:
    if len(sys.argv) != 2:
        sys.stderr.write("usage: fit_analysis.py <firmware_i4004.ll>\n")
        return 2
    ll = open(sys.argv[1], encoding="utf-8").read()

    globals_ram = []
    for m in re.finditer(r"^(@[\w.]+)\s*=\s*([^\n]+)$", ll, flags=re.M):
        name, rest = m.group(1), m.group(2)
        if " constant " in rest:
            continue
        if "global " in rest:
            tym = re.search(r"global\s+(\[[^\]]+\]|[^\s,]+)", rest)
            sz = ty_size(tym.group(1)) if tym else 0
            globals_ram.append((name, sz))

    frames = defaultdict(int)
    cur = None
    for line in ll.splitlines():
        d = re.match(r"^define\b.*?(@[\w.]+|@\"[^\"]+\")\s*\(", line)
        if d:
            cur = d.group(1)
            continue
        if line.startswith("}"):
            cur = None
            continue
        a = re.search(r"=\s*alloca\s+([^\s,]+)", line)
        if a and cur:
            frames[cur] += ty_size(a.group(1))

    deref = sorted(
        {int(x) for x in re.findall(r"dereferenceable\((\d+)\)", ll)}, reverse=True
    )
    n_def = len(re.findall(r"^define ", ll, flags=re.M))
    insns = len(re.findall(r"^\s+[%@a-zA-Z].*$", ll, flags=re.M))

    print("=== RAM: static module globals (non-constant) ===")
    if globals_ram:
        for nm, sz in globals_ram:
            print(f"  {nm}: {sz} bytes")
        print(f"  total static: {sum(s for _, s in globals_ram)} bytes")
    else:
        print("  (none - all state is threaded through locals from main)")

    print("\n=== RAM: per-function alloca frames (bytes) ===")
    for fn, sz in sorted(frames.items(), key=lambda x: -x[1]):
        if sz:
            print(f"  {sz:5d}  {fn[:70]}")

    print("\n=== Largest in-memory objects (dereferenceable hints, bytes) ===")
    print(" ", deref[:12])

    print("\n=== ROM proxy ===")
    print(f"  defined functions: {n_def}")
    print(f"  IR instruction lines (rough): {insns}")

    main_mem = 64
    biggest = deref[0] if deref else 0
    print("\n=== FIT vs stock board (64 bytes main memory) ===")
    print(f"  biggest single object: {biggest} bytes")
    if biggest > main_mem:
        print(f"  >>> does NOT fit: one object alone is {biggest} > {main_mem} bytes")
    else:
        print("  one object fits; check simultaneous-frame sum next")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
