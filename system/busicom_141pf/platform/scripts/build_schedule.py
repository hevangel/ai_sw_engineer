#!/usr/bin/env python3
"""Build the deterministic key/switch schedule replayed on every busicom_141pf
simulator from the committed manual examples.

The schedule is the single source of inputs for BOTH the chips (secondary) and
the RTL (primary) runners, so the two simulators are fed byte-for-byte identical
stimulus. Format (one token line per event, blank-free):

    #EXAMPLE <id>
    2 0 0 2000                       reset settle (precision rounding key duration)
    <precision> <rounding> <key> 400 one key press
    #RESET                           checkpoint marker after setup_keys
    <precision> <rounding> <key> 400
    #STEP <n>                        checkpoint marker after step n's keys

This mirrors ai_hw_engineer/scripts/check_reference.py exactly (same replay
semantics: set switches, press setup_keys = CE C CM C, then per step apply
switches and press keys, marking #RESET/#STEP checkpoints). Derived facts only;
no third-party code is copied.
"""
import argparse
import json
import sys


def build(doc):
    lines = []
    for example in doc["examples"]:
        lines.append("#EXAMPLE " + example["id"])
        lines.append("2 0 0 2000")
        switches = dict(example["switches"])
        setup = {"keys": example["setup_keys"]}
        for index, step in enumerate([setup] + example["steps"]):
            switches.update(step.get("switches", {}))
            for key in step.get("keys", []):
                code = doc["key_codes"][key]
                lines.append(f"{switches['precision']} {switches['rounding']} {code} 400")
            lines.append("#RESET" if index == 0 else "#STEP " + str(index - 1))
    return "\n".join(lines) + "\n"


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("json", help="manual_examples.json")
    ap.add_argument("-o", "--out", help="output schedule (default stdout)")
    args = ap.parse_args()
    with open(args.json, encoding="utf-8") as fh:
        doc = json.load(fh)
    if doc.get("schema_version") != 1:
        sys.exit("unsupported schema_version")
    text = build(doc)
    if args.out:
        with open(args.out, "w", encoding="utf-8", newline="\n") as fh:
            fh.write(text)
    else:
        sys.stdout.write(text)


if __name__ == "__main__":
    main()
