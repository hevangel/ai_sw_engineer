#!/usr/bin/env python3
"""Score a produced busicom_141pf trace against an oracle example document.

The generic companion to `build_schedule.py`: it replays-checks any trace
produced from that schedule (emulator CLI, RTL runner, port-level capture)
against the per-step expected tape suffixes and lamps recorded in an oracle
document such as `firmware/oracle/random_calculator_tests.json`.

Comparison rules (mirroring the Rust oracles):

- Payload lines are `#EXAMPLE`/`#RESET`/`#STEP`/`LAMPS`/`ROW`.
- Rows buffered since the last marker belong to the step whose `#STEP i`
  marker comes next; that marker's `LAMPS|n` line is the end-of-step state.
- A row `ROW|value|symA|symB|red` folds to the oracle token exactly like
  `firmware/src/print.rs` `Row::oracle_symbol`; the dotted overflow line
  (15 `.` columns) maps to `value: null, symbol: "overflow"`.
- The expected tape must be an exact ordered suffix of the step's rows;
  `expect.lamps` entries compare against bits 0..2 of the lamp nibble.

Usage: compare_oracle.py <produced_trace.txt> <oracle.json>
Exit 0 iff every step expectation holds.
"""
import argparse
import json
import sys

DOTS = "." * 15


def oracle_symbol(sym_a, sym_b):
    for pair, token in [
        (("C", "M"), "CM"), (("R", "M"), "RM"),
        ((None, "*"), "*"), ((None, "M+"), "M+"), ((None, "M-"), "M-"),
        ((None, "#"), "#"), ((None, "Ex"), "EX"), ((None, "M"), "M"),
        (("◇", None), "ST"), (("+", None), "+"), (("-", None), "-"),
        (("×", None), "*op"), (("÷", None), "/"),
        (("M+", None), "M+"), (("M-", None), "M-"),
        (("=", None), "="), (("√", None), "SQRT"), (("%", None), "%"),
        (("C", None), "C"), (("R", None), "R"),
    ]:
        a, b = pair
        if a is not None and sym_a != a:
            continue
        if b is not None and sym_b != b:
            continue
        return token
    return ""


def parse_row(line):
    fields = line.split("|")
    value, sym_a, sym_b, red = fields[1], fields[2], fields[3], fields[4] == "true"
    if value == DOTS:
        return {"value": None, "symbol": "overflow", "red": False, "rounded": False}
    return {
        "value": value.rstrip() or "",
        "symbol": oracle_symbol(sym_a, sym_b),
        "red": red,
        "rounded": sym_a == "^",
    }


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("trace")
    ap.add_argument("oracle_json")
    args = ap.parse_args()

    with open(args.oracle_json, encoding="utf-8") as fh:
        doc = json.load(fh)
    examples = {ex["id"]: ex for ex in doc["examples"]}

    failures = []
    rows_checked = lamp_checks = steps_checked = 0
    steps = None
    example_id = None
    current = 0
    pending = []
    lamps_pending = None

    with open(args.trace, encoding="utf-8") as fh:
        for line in fh:
            line = line.rstrip("\n")
            if line.startswith("#EXAMPLE "):
                example_id = line[len("#EXAMPLE "):]
                steps = examples[example_id]["steps"]
                current = 0
                pending = []
                lamps_pending = None
            elif line == "#RESET":
                pending = []
            elif line.startswith("#STEP "):
                n = int(line[len("#STEP "):])
                if n != current:
                    failures.append(f"{example_id}: unexpected checkpoint {n} (at {current})")
                    break
                expect = steps[current].get("expect")
                if expect:
                    steps_checked += 1
                    tape = expect.get("tape")
                    if tape:
                        rows_checked += len(tape)
                        if len(pending) < len(tape) or pending[-len(tape):] != tape:
                            failures.append(
                                f"{example_id} step {current + 1}: expected tape suffix "
                                f"{tape}, got {pending}"
                            )
                    lamps = expect.get("lamps")
                    if lamps:
                        lamps_pending = (current, lamps)
                pending = []
                current += 1
            elif line.startswith("LAMPS|"):
                if lamps_pending is not None:
                    idx, lamps = lamps_pending
                    lamps_pending = None
                    lamp_checks += 1
                    v = int(line[len("LAMPS|"):])
                    got = {"memory": bool(v & 1), "overflow": bool(v & 2),
                           "negative": bool(v & 4)}
                    for name, want in lamps.items():
                        if got[name] != want:
                            failures.append(
                                f"{example_id} step {idx + 1}: lamp {name} "
                                f"expected {want}, got {got[name]}"
                            )
            elif line.startswith("ROW|"):
                pending.append(parse_row(line))

    total = sum(1 for ex in doc["examples"] for st in ex["steps"] if st.get("expect"))
    if steps_checked != total:
        failures.append(f"evaluated {steps_checked} steps, document has {total}")
    if failures:
        print(f"FAIL: {len(failures)} problem(s):")
        for f in failures[:10]:
            print("  " + f)
        return 1
    print(f"PASS: {len(doc['examples'])}/{len(doc['examples'])} examples "
          f"({steps_checked} steps, {rows_checked} rows, {lamp_checks} lamp checkpoints)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
