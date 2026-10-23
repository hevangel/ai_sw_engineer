#!/usr/bin/env python3
"""Rank the operations our llc cannot yet select, across the whole firmware.

`llc` aborts on the FIRST unselectable DAG node, so a single run over the whole
module only ever names one problem. To get a ranked work list we split the
retargeted module into one module per defined function (the shared preamble —
target header, types, globals, `declare`s, attribute groups, metadata — plus a
`declare` for every OTHER function, plus exactly one real `define`) and run llc
on each in isolation, then aggregate the failures.

For each function llc either succeeds (that function is already fully
selectable) or aborts with "LLVM ERROR: Cannot select: <node>" (or a legalize /
other backend error). We normalise each failing node to its operation
signature (drop SSA ids, addresses, constants) and count how many functions hit
it. The ranked list is the backlog the codegen agent burns down.

It also runs the whole-program SIZE GATE: the module is compiled once with
`-function-sections -filetype=obj`, every `.text.<fn>` section is measured
(llvm-objdump -h), and the run FAILS (exit 3) if the summed `.text` exceeds
ROM_BUDGET or any single function exceeds FN_MAX. The top-10 functions by size
are printed and written to the report. This is the gate the 2026-10-19 failure
note asked for: it runs from the first function that lowers, long before an
image links.

Usage:
  gen_worklist.py <firmware_i4004.ll> <llc-path> <out.md> [--triple i4004]

Environment:
  ROM_BUDGET  summed .text limit in bytes (default 4096, the 16 x 4001 ceiling)
  FN_MAX      per-function limit in bytes (default 1024)
  SIZE_GATE   enforce (default) | report  -- report never fails the run
  LLVM_OBJDUMP  objdump path (default: next to <llc-path>)
"""
from __future__ import annotations

import re
import subprocess
import sys
import tempfile
from collections import Counter, defaultdict
from pathlib import Path


# Per-function llc timeout (seconds). llc can enter an infinite legalize/
# combine/isel loop on some IR; a run exceeding this is treated as a backend
# bug ('hang'), never waited on. Override with LLC_TIMEOUT in the environment.
import os
LLC_TIMEOUT = int(os.environ.get("LLC_TIMEOUT", "30"))


def normalise_node(msg: str) -> str:
    """Reduce a 'Cannot select' node line to an op signature for counting."""
    s = msg.strip()
    s = re.sub(r"0x[0-9a-fA-F]+", "", s)
    s = re.sub(r"\bt\d+\b", "t", s)
    s = re.sub(r"<[^>]*>", "", s)
    s = re.sub(r"Constant:i\d+", "Constant", s)
    s = re.sub(r"%\S+", "%v", s)
    s = re.sub(r"\s+", " ", s).strip()
    return s


def extract_failure(stderr: str):
    """Return (category, op, normalised_signature) from an llc failure, or None."""
    for line in stderr.splitlines():
        if "Cannot select:" in line:
            node = line.split("Cannot select:", 1)[1]
            # node looks like: "tNN: <type> = <OP> <operands>" — the op is the
            # mnemonic right after the first '='. Allow upper/lower/underscore
            # (e.g. brcond, Constant, zero_extend, GlobalAddress).
            m = re.search(r"=\s*([A-Za-z_][A-Za-z0-9_]*)", node)
            op = m.group(1) if m else (node.strip().split() or ["?"])[0].rstrip(":")
            return ("cannot-select", op, normalise_node(node))
    low = stderr.lower()
    if "unable to legalize" in low:
        return ("legalize", "legalize", normalise_node(stderr))
    # Backend assertion (e.g. SELECT/SELECT_CC expand) — an incomplete lowering,
    # not a clean "Cannot select". Pull the asserted expression as the op.
    for line in stderr.splitlines():
        if "Assertion `" in line or "assert(" in line:
            m = re.search(r"ISD::([A-Za-z_]+)", line)
            op = f"assert:{m.group(1)}" if m else "assert"
            return ("backend-assert", op, normalise_node(line))
    # A hard crash with no diagnostic (e.g. missing AsmPrinter mnemonic throwing
    # std::logic_error) — still a backend gap, bucket it so it is not lost.
    if "terminate called" in stderr or "logic_error" in stderr:
        return ("backend-crash", "crash:asmprinter", "std::logic_error during emission")
    for line in stderr.splitlines():
        if "LLVM ERROR:" in line:
            return ("error", "llvm-error", normalise_node(line.split("LLVM ERROR:", 1)[1]))
    return None


ROM_BUDGET = int(os.environ.get("ROM_BUDGET", "4096"))
FN_MAX = int(os.environ.get("FN_MAX", "1024"))
SIZE_GATE = os.environ.get("SIZE_GATE", "enforce")


def measure_text(ll_path: str, llc: str, objdump: str, triple: str):
    """Compile the whole module once; return [(function, bytes)] largest first.

    With -function-sections each function lands in `.text.<symbol>`, so the
    section table is the per-function size list. Returns None if the module
    does not compile as a whole (the per-function pass above reports why).
    """
    with tempfile.TemporaryDirectory() as td:
        obj = Path(td) / "fw.o"
        r = subprocess.run(
            [llc, "-mtriple=" + triple, "-function-sections", "-filetype=obj",
             ll_path, "-o", str(obj)],
            capture_output=True, text=True, timeout=LLC_TIMEOUT * 10,
        )
        if r.returncode != 0:
            return None
        h = subprocess.run([objdump, "-h", str(obj)], capture_output=True,
                           text=True, check=True).stdout
    sizes = []
    for line in h.splitlines():
        m = re.match(r"\s*\d+\s+(\.text\S*)\s+([0-9a-fA-F]+)\s", line)
        if m and int(m.group(2), 16):
            name = m.group(1)[len(".text."):] if m.group(1) != ".text" else ".text"
            sizes.append((name, int(m.group(2), 16)))
    sizes.sort(key=lambda t: -t[1])
    return sizes


def demangle_hint(sym: str) -> str:
    """Best-effort readable hint from a Rust mangled name (no rustc needed)."""
    s = sym.lstrip("@").strip('"')
    if s.startswith("_R"):
        # v0 mangling: walk the structure, keeping length-prefixed identifiers
        # and skipping `s<base62>_` disambiguators and `B<base62>_` backrefs
        # (whose base62 digits would otherwise read as identifier lengths).
        idents, i = [], 2
        while i < len(s):
            c = s[i]
            if c.isdigit():
                j = i
                while j < len(s) and s[j].isdigit():
                    j += 1
                n = int(s[i:j])
                if j < len(s) and s[j] == "_":  # `_` separates a leading digit
                    j += 1
                idents.append(s[j:j + n])
                i = j + n
            elif c in "sB":
                k = s.find("_", i)
                i = len(s) if k < 0 else k + 1
            else:
                i += 1
        # Drop the crate names (firmware/software) for brevity.
        idents = [t for t in idents if t not in ("software", "firmware", "target")]
        if idents:
            return "::".join(idents)
    toks = re.findall(r"[0-9]+([A-Za-z_][A-Za-z0-9_]*)", s)
    hint = "::".join(t for t in toks if not t.startswith("Cs"))
    return hint or s


def stub_body(define_line: str, body: str) -> str:
    """Replace a function body with a trivial, always-selectable stub.

    Keeps the full `define ...` header (linkage, calling convention, attribute
    refs, metadata) so every call site stays valid, but discards the real
    instructions so this function never contributes an ISel failure of its own.
    The body is a single `unreachable`, which is valid for ANY return type and
    needs no typed return value, so no per-type value synthesis is required.
    This keeps the module well-formed (unlike turning the define into a declare,
    which breaks `internal`/`fastcc` call sites) while isolating the function
    under test.
    """
    header = define_line.rsplit("{", 1)[0].rstrip()
    return header + " {\nentry:\n  unreachable\n}\n"


def split_functions(ll: str):
    """Return (preamble_non_define_lines, [(name, define_line, body_text), ...])."""
    lines = ll.splitlines(keepends=True)
    bodies = []
    preamble = []
    i, n = 0, len(lines)
    while i < n:
        line = lines[i]
        if line.startswith("define "):
            depth = line.count("{") - line.count("}")
            j = i
            while depth > 0 and j + 1 < n:
                j += 1
                depth += lines[j].count("{") - lines[j].count("}")
            m = re.search(r'(@[A-Za-z0-9_.]+|@"[^"]+")\s*\(', line)
            name = m.group(1) if m else f"@fn{i}"
            bodies.append((name, line, "".join(lines[i : j + 1])))
            i = j + 1
        else:
            preamble.append(line)
            i += 1
    return "".join(preamble), bodies


def main() -> int:
    if len(sys.argv) < 4:
        sys.stderr.write(__doc__)
        return 2
    ll_path, llc, out_md = sys.argv[1], sys.argv[2], sys.argv[3]
    triple = "i4004"
    if "--triple" in sys.argv:
        triple = sys.argv[sys.argv.index("--triple") + 1]

    ll = Path(ll_path).read_text(encoding="utf-8")
    preamble_text, bodies = split_functions(ll)

    ok, failed = [], []
    reasons = Counter()
    ops = Counter()
    op_funcs = defaultdict(list)

    with tempfile.TemporaryDirectory() as td:
        for idx, (name, define_line, body) in enumerate(bodies):
            others = "".join(
                stub_body(dl, b) for (nm, dl, b) in bodies if nm != name
            )
            mod = preamble_text + "\n" + others + "\n" + body
            p = Path(td) / f"fn{idx}.ll"
            p.write_text(mod, encoding="utf-8")
            try:
                r = subprocess.run(
                    [llc, "-mtriple=" + triple, "-filetype=asm", str(p), "-o", str(p) + ".s"],
                    capture_output=True,
                    text=True,
                    timeout=LLC_TIMEOUT,
                )
            except subprocess.TimeoutExpired:
                # llc looping on this function (legalize/combine/isel cycle) is a
                # backend BUG, recorded as a 'hang' failure -- never waited on.
                reasons["hang"] += 1
                ops["hang:infinite-loop"] += 1
                op_funcs["hang:infinite-loop"].append(name)
                failed.append((name, "hang:infinite-loop",
                               f"llc exceeded {LLC_TIMEOUT}s (infinite loop)"))
                continue
            if r.returncode == 0:
                ok.append(name)
                continue
            fx = extract_failure(r.stderr)
            if fx is None:
                reasons["unknown"] += 1
                tail = r.stderr.strip().splitlines()[-1] if r.stderr.strip() else "?"
                # Record under an 'unknown' op bucket so the function names and a
                # representative message are still emitted (otherwise an
                # unclassified failure is counted but never attributed).
                ops["unknown"] += 1
                op_funcs["unknown"].append(name)
                failed.append((name, "unknown", tail))
                # Persist the full stderr next to the IR for post-mortem.
                try:
                    dbg = Path(out_md).with_name("worklist_unknown.log")
                    with dbg.open("a", encoding="utf-8") as fh:
                        fh.write(f"===== {name} =====\n{r.stderr}\n")
                except Exception:
                    pass
            else:
                cat, op, sig = fx
                reasons[cat] += 1
                ops[op] += 1
                op_funcs[op].append(name)
                failed.append((name, op, sig))

    objdump = os.environ.get("LLVM_OBJDUMP",
                             str(Path(llc).with_name("llvm-objdump")))
    sizes = measure_text(ll_path, llc, objdump, triple)
    gate_fail = []
    if sizes is None:
        gate_fail.append("whole module does not compile; .text not measurable")
    else:
        text = sum(n for _, n in sizes)
        if text > ROM_BUDGET:
            gate_fail.append(f".text {text} B > ROM_BUDGET {ROM_BUDGET} B "
                             f"({text / ROM_BUDGET:.1f}x)")
        big = [(f, n) for f, n in sizes if n > FN_MAX]
        if big:
            gate_fail.append(f"{len(big)} function(s) > FN_MAX {FN_MAX} B")

    total = len(bodies)
    md = []
    md.append("# i4004 codegen work list (from the firmware whole-program IR)\n")
    md.append(
        "Generated by `system/busicom_141pf/firmware/scripts/gen_worklist.py`: it "
        "splits `build/firmware_i4004.ll` into one module per defined function and "
        "runs our `llc -mtriple=i4004` on each in isolation (llc aborts on the "
        "first unselectable node, so per-function isolation is the only way to see "
        "them all). The ranking is what the codegen backend must support for the "
        "firmware to lower end to end.\n"
    )
    md.append(f"- Functions in module: **{total}**")
    md.append(f"- Select cleanly today: **{len(ok)}**")
    md.append(f"- Fail to select: **{len(failed)}**\n")

    md.append("## Size gate\n")
    if sizes is None:
        md.append("- whole module does not compile; `.text` not measurable\n")
    else:
        text = sum(n for _, n in sizes)
        md.append(f"- `.text` (sum of {len(sizes)} sections): **{text} B** "
                  f"vs ROM_BUDGET {ROM_BUDGET} B ({text / ROM_BUDGET:.2f}x); "
                  f"vs the 1,280 B original ROM {text / 1280:.1f}x")
        md.append(f"- Largest function: **{sizes[0][1]} B** (FN_MAX {FN_MAX} B)")
        md.append(f"- Gate: **{'FAIL' if gate_fail else 'pass'}**"
                  + (f" — {'; '.join(gate_fail)}" if gate_fail else "") + "\n")
        md.append("| Rank | Function | Bytes |")
        md.append("|---|---|---|")
        for rank, (fn, n) in enumerate(sizes[:10], 1):
            md.append(f"| {rank} | `{demangle_hint(fn)}` | {n} |")
        md.append("")

    md.append("## Ranked by failing operation (functions blocked)\n")
    md.append("| Rank | Operation (first unselected node) | Functions blocked |")
    md.append("|---|---|---|")
    for rank, (op, cnt) in enumerate(ops.most_common(), 1):
        md.append(f"| {rank} | `{op}` | {cnt} |")
    if not ops:
        md.append("| — | (none) | 0 |")
    md.append("")
    md.append(
        "> Note: each function reports only its FIRST unselected node, so an op "
        "lower in the list can also be needed by a function blocked earlier on a "
        "higher one. Re-run after each codegen landing to reveal the next layer.\n"
    )

    md.append("## Failure categories\n")
    for cat, cnt in reasons.most_common():
        md.append(f"- `{cat}`: {cnt}")
    if not reasons:
        md.append("- (none)")
    md.append("")

    md.append("## Per-operation detail\n")
    for op, cnt in ops.most_common():
        md.append(f"### `{op}` — {cnt} function(s)\n")
        rep = next((sig for (nm, o, sig) in failed if o == op), "")
        if rep:
            md.append("Representative unselected node:\n")
            md.append("```")
            md.append(rep)
            md.append("```\n")
        md.append("<details><summary>functions</summary>\n")
        for nm in op_funcs[op][:50]:
            md.append(f"- `{demangle_hint(nm)}`")
        md.append("\n</details>\n")

    if ok:
        md.append("## Functions that already select cleanly\n")
        for nm in ok:
            md.append(f"- `{demangle_hint(nm)}`")
        md.append("")

    Path(out_md).write_text("\n".join(md) + "\n", encoding="utf-8")
    sys.stderr.write(
        f"gen_worklist: {len(ok)}/{total} select, {len(failed)} fail; "
        f"top ops: {', '.join(f'{o}({c})' for o, c in ops.most_common(5)) or 'none'}\n"
    )
    if sizes is not None:
        sys.stderr.write(f"size gate: .text {sum(n for _, n in sizes)} B "
                         f"(ROM_BUDGET {ROM_BUDGET}, FN_MAX {FN_MAX}); top 10:\n")
        for fn, n in sizes[:10]:
            sys.stderr.write(f"  {n:6d}  {demangle_hint(fn)}\n")
    if gate_fail:
        sys.stderr.write("SIZE GATE FAIL: " + "; ".join(gate_fail) + "\n")
        if SIZE_GATE != "report":
            return 3
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
