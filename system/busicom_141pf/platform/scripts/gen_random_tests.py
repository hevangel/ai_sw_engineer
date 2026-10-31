#!/usr/bin/env python3
"""Generate seeded random Busicom 141-PF calculator programs.

Writes `firmware/oracle/random_calculator_tests.json`: COUNT examples whose
steps carry key sequences and the number of tail rows to assert
(`assert_rows`), but NO expected tape. Expected rows are derived afterwards
by the firmware host core (`firmware/tests/random_calculator_tests.rs`,
`dump_random_oracle`), which independently reproduces the committed
original-ROM oracle 42/42; they are never derived from the emulator under
test.

Every program shape is composed from behaviour anchored by
`firmware/oracle/manual_examples.json` (all 42 manual examples) and the
full-row port trace (`firmware/oracle/rom_port_trace.txt`):

- add/subtract chains with repeated entries, subtotals (ST), non-add `#`
  entries, a decimal-selector change before `=`, and negative grand totals;
- multiply/divide with `=`, constant operands, repeated-`=` powers, chain
  prints with ST, op-key correction, EX operand exchange;
- percentage multiply and divide, square root of an entry and of a result;
- memory: bare `M+`/`M-`, `M=+`/`M=-` constant chains, RM/CM with the
  memory lamp (5/4 rounding, as in the manual sources);
- the documented capacity overflows (addition entry, addition accumulator
  with CE recovery, multiplication, division).

Operand sizes are bounded so no capacity limit is crossed except in the
deliberate overflow shapes. Selector eligibility follows the anchors:
memory blocks and division-by-recalled-memory run at 5/4, overflow
recovery at IN with DP=8 (manual 14-3), and add/subtract chains that end
in `=` never use FL (no manual example anchors FL totals).

The generator is deterministic: same seed, same corpus. `--check`
regenerates in memory and compares the committed JSON's programs against
the seed, failing on any drift.
"""
import argparse
import json
import random
import sys
from decimal import Decimal
from pathlib import Path

SEED = 20261025
COUNT = 1000
SETUP_KEYS = ["CE", "C", "CM", "C"]

HERE = Path(__file__).resolve().parent
ORACLE = HERE.parent.parent / "firmware" / "oracle"

MEMORY_KEYS = {"M+", "M-", "M=+", "M=-", "RM", "CM"}


def entry(rng, int_digits, frac_digits, leading_point=False):
    """A number-entry key list: digits, optional `.` + fraction.

    The integer part never has a leading zero; with `leading_point` and a
    fraction, the entry may type `.xxx` (manual 1-1 step 5 prints `0.123`).
    An all-zero entry is redrawn: zero operands print at scales where the
    host core and the ROM still disagree (see the failure note referenced
    in [`MulBlock`]).
    """
    while True:
        digits = [str(rng.randint(1, 9))] + [
            str(rng.randint(0, 9)) for _ in range(int_digits - 1)
        ]
        frac = [str(rng.randint(0, 9)) for _ in range(frac_digits)]
        if frac_digits > 0 and leading_point and rng.random() < 0.5:
            # `.xxx` drops the integer part: the fraction must be nonzero.
            if any(d != "0" for d in frac):
                return ["."] + frac
            continue
        if not any(d != "0" for d in digits) and not any(d != "0" for d in frac):
            continue
        keys = list(digits)
        if frac_digits > 0:
            keys.append(".")
            keys.extend(frac)
        return keys


def int_entry(rng, int_digits):
    return [str(rng.randint(1, 9))] + [str(rng.randint(0, 9)) for _ in range(int_digits - 1)]


class AddBlock:
    """M1 addition/subtraction chain (manual 1-1..1-4, 14-1..14-3)."""

    def __init__(self, rng, dp, rounding, with_total, allow_dp_change=False):
        self.dp = dp
        self.with_total = with_total
        # Aligned operand/result width stays <= 14 printed digits: int <= 13-dp
        # (margin 1 below the 14-dp limit; dp=8 caps int at 5 like manual 14-3).
        self.int_cap = 5 if dp == 8 else 13 - dp
        self.bound = 0
        self.max_ia = 1
        self.n = rng.randint(2, 4)
        self.subtotal = rng.random() < 0.35
        if self.subtotal:
            self.ops = ["+"] * self.n  # 1-4 anchors ST on positive subtotals
        else:
            self.ops = [rng.choice("+-") for _ in range(self.n)]
        self.repeat = rng.random() < 0.25
        self.nonadd = rng.random() < 0.2
        self.nonadd_value = rng.randint(1, 9999)
        # The selector stays where a step leaves it, so a DP change is only
        # safe in the example's final block.
        self.dp_change = (
            with_total and allow_dp_change
            and rounding != 0 and dp >= 2 and rng.random() < 0.25
        )
        # The selector has no 7 position (0..6 or 8): step down within range.
        self.dp2 = dp - rng.randint(1, 2) if self.dp_change else dp
        if self.dp2 == 7:
            self.dp2 = 6

    def build(self, rng):
        steps = []
        if self.nonadd:
            # 1-4 step 0: a non-add `#` entry comes before any additions.
            steps.append({
                "keys": [str(d) for d in str(self.nonadd_value)] + ["ST"],
                "assert_rows": 1,
            })
        # Exact operand values (the generator knows them), so zero grand
        # totals can be avoided: a zero total prints at a scale the core and
        # the ROM disagree on (see the failure note referenced in [`MulBlock`]).
        steps = []
        if self.nonadd:
            # 1-4 step 0: a non-add `#` entry comes before any additions.
            steps.append({
                "keys": [str(d) for d in str(self.nonadd_value)] + ["ST"],
                "assert_rows": 1,
            })
        values = []

        def sample(ia, sf):
            for _ in range(8):
                keys = entry(rng, ia, sf, leading_point=True)
                v = Decimal("".join(keys))
                if self.ops[min(len(values), self.n - 1)] == "-":
                    v = -v
                if not self.with_total or sum(values) + v != 0:
                    return keys, v
            return None, None

        for i in range(self.n):
            ia = rng.randint(1, min(6, self.int_cap))
            sf = rng.randint(0, self.dp) if self.dp > 0 else 0
            keys, v = sample(ia, sf)
            if keys is None:
                return []
            steps.append({"keys": keys + [self.ops[i]], "assert_rows": 1})
            values.append(v)
            self.bound += 10**ia
            self.max_ia = max(self.max_ia, ia)
        if self.repeat:
            last = values[-1]
            if sum(values) + last != 0:
                steps.append({"keys": [self.ops[-1]], "assert_rows": 1})
                values.append(last)
                self.bound += 10**self.max_ia
        if self.subtotal and sum(values) > 0 and self.bound <= 10**self.int_cap:
            # 1-4: subtotal ST directly after an add/subtract operand step.
            steps.append({"keys": ["ST"], "assert_rows": 1})
        if self.with_total:
            if self.dp_change:
                steps.append({"switches": {"precision": self.dp2}, "assert_rows": 0})
            steps.append({"keys": ["="], "assert_rows": 1})
        return steps


class MulBlock:
    """M2 multiplication (manual 2-2, 2-3, 2-5..2-8, 15-1..15-4).

    Restricted to 5/4 and IN: the ROM's floating-precision result width
    (13 vs 14 digits) still diverges from the host core on wide products,
    so FL operands are out of scope until the core matches the ROM there
    (see failure_notes/2026-10-25-busicom-141pf-random-corpus-divergences.md).
    """

    def __init__(self, rng, dp, rounding, kind):
        assert rounding in (1, 8), "FL mul excluded from this corpus"
        self.dp, self.rounding, self.kind = dp, rounding, kind
        # Fixed modes print at DP (width int+dp <= 14); stay one digit inside.
        self.cap = max(2, 12 - dp)

    def operand(self, rng, ia_cap=None):
        ia = rng.randint(1, max(1, min(ia_cap or self.cap, self.cap // 2 + 1)))
        sf = rng.randint(0, min(self.dp, 3)) if self.dp > 0 else 0
        return entry(rng, ia, sf, leading_point=True), ia + sf

    def build(self, rng):
        steps = []
        if self.kind == "simple":
            a, used = self.operand(rng)
            b, _ = self.bounded(rng, used)
            steps.append({"keys": a + ["*"], "assert_rows": 1})
            steps.append({"keys": b + ["="], "assert_rows": 2})
        elif self.kind == "constant":
            a, used = self.operand(rng)
            steps.append({"keys": a + ["*"], "assert_rows": 1})
            for _ in range(rng.randint(2, 3)):
                if used >= 12:
                    break
                c, w = self.bounded(rng, used)
                steps.append({"keys": c + ["="], "assert_rows": 2})
                used += w
        elif self.kind == "powers":
            a, w = self.operand(rng, ia_cap=2)
            steps.append({"keys": a + ["*"], "assert_rows": 1})
            for _ in range(min(3, max(1, 12 // w))):
                steps.append({"keys": ["="], "assert_rows": 2})
        elif self.kind == "chain_st":
            a, used = self.operand(rng)
            b, w = self.bounded(rng, used)
            used += w
            c, _ = self.bounded(rng, used)
            steps.append({"keys": a + ["*"], "assert_rows": 1})
            steps.append({"keys": b + ["*"], "assert_rows": 0})
            steps.append({"keys": ["ST"], "assert_rows": 1})
            steps.append({"keys": c + ["="], "assert_rows": 2})
        elif self.kind == "correction":
            a, _ = self.operand(rng)
            b, _ = self.bounded(rng, 0)
            first, second = rng.sample(["*", "/"], 2)
            steps.append({"keys": a + [first, second, second], "assert_rows": 1})
            steps.append({"keys": b + ["="], "assert_rows": 2})
        elif self.kind == "ex":
            a, used = self.operand(rng)
            b, w = self.bounded(rng, used)
            used += w
            c, _ = self.bounded(rng, used)
            steps.append({"keys": a + ["*"], "assert_rows": 1})
            steps.append({"keys": b + ["EX"], "assert_rows": 1})
            steps.append({"keys": ["="], "assert_rows": 2})
            steps.append({"keys": c + ["="], "assert_rows": 2})
        return steps

    def bounded(self, rng, used):
        """Draw a second operand whose combined exact width stays <= 12, so
        no exact product crosses the 13-digit region where the host core and
        the ROM still disagree (see the failure note referenced above)."""
        budget = max(1, 12 - used)
        ia = rng.randint(1, max(1, min(self.cap // 2 + 1, budget)))
        sf = rng.randint(0, max(0, min(self.dp, 3, budget - ia)))
        return entry(rng, ia, sf, leading_point=True), ia + sf


class DivBlock:
    """M2 division (manual 3-2, 3-3, 3-5, 3-6).

    Restricted to 5/4 and IN: the ROM's floating-precision quotient width
    still diverges from the host core (see the failure note referenced in
    [`MulBlock`]).
    """

    def __init__(self, rng, dp, rounding, kind):
        assert rounding in (1, 8), "FL div excluded from this corpus"
        self.dp, self.rounding, self.kind = dp, rounding, kind
        # Quotient integer digits stay <= divisor-shifted dividend size;
        # keep dividend int <= 12-dp.
        self.cap = max(3, 12 - dp)

    def dividend(self, rng, cap=None):
        ia = rng.randint(2, min(cap or self.cap, self.cap))
        sf = rng.randint(0, min(self.dp, 3)) if self.dp > 0 else 0
        return entry(rng, ia, sf, leading_point=True)

    def divisor(self, rng):
        # Never zero: integer part >= 1, or a lone fractional tail.
        if self.dp > 0 and rng.random() < 0.4:
            return entry(rng, 1, rng.randint(1, min(self.dp, 6)))
        return [str(rng.randint(1, 9))]

    def build(self, rng):
        steps = []
        if self.kind == "simple":
            d = self.dividend(rng)
            steps.append({"keys": d + ["/"], "assert_rows": 1})
            steps.append({"keys": self.divisor(rng) + ["="], "assert_rows": 2})
        elif self.kind == "constant":
            steps.append({"keys": self.dividend(rng, self.cap - 4) + ["/"], "assert_rows": 1})
            for _ in range(rng.randint(2, 3)):
                steps.append({"keys": self.divisor(rng) + ["="], "assert_rows": 2})
        elif self.kind == "chain":
            steps.append({"keys": self.dividend(rng, self.cap - 4) + ["/"], "assert_rows": 1})
            steps.append({"keys": self.divisor(rng) + ["/"], "assert_rows": 0})
            steps.append({"keys": self.divisor(rng) + ["="], "assert_rows": 2})
        elif self.kind == "repeat_eq":
            # 11-2 step 13: a second '=' divides by the held constant again.
            steps.append({"keys": self.dividend(rng, self.cap - 4) + ["/"], "assert_rows": 1})
            steps.append({"keys": self.divisor(rng) + ["="], "assert_rows": 2})
            steps.append({"keys": ["="], "assert_rows": 2})
        elif self.kind == "mem_dividend":
            # 3-6: constant dividend held in memory (5/4 rounding). The M+
            # entry is DP-aligned, so its fraction stays within DP (A1).
            sf = min(rng.randint(0, 2), self.dp) if self.dp > 0 else 0
            steps.append({"keys": entry(rng, rng.randint(2, 6), sf) + ["M+"],
                          "assert_rows": 1})
            for _ in range(rng.randint(2, 3)):
                steps.append({"keys": ["/"], "assert_rows": 1})
                steps.append({"keys": self.divisor(rng) + ["="], "assert_rows": 2})
                steps.append({"keys": ["RM"], "assert_rows": 1})
        return steps


class PercentBlock:
    """Percentage multiply/divide (manual 4-1, 4-2)."""

    def __init__(self, rng, dp, rounding, kind):
        self.dp, self.kind = dp, kind
        self.cap = max(3, 10 - dp)

    def build(self, rng):
        steps = []
        base = entry(rng, rng.randint(2, self.cap), rng.randint(0, 2) if self.dp > 0 else 0)
        if rng.random() < 0.5:
            pct = [str(rng.randint(1, 9))]
        else:
            pct = [str(rng.randint(1, 9)), str(rng.randint(0, 9))]
        op = "*" if self.kind == "mul" else "/"
        steps.append({"keys": base + [op], "assert_rows": 1})
        steps.append({"keys": pct + ["%"], "assert_rows": 2})
        return steps


class SqrtBlock:
    """Square root (manual 11-1, 11-2 step 14)."""

    def __init__(self, rng, dp, rounding, kind):
        self.dp, self.kind = dp, kind

    def build(self, rng):
        steps = []
        if self.kind == "entry":
            rad = entry(rng, rng.randint(1, 6), rng.randint(0, min(self.dp, 4)) if self.dp > 0 else 0)
            steps.append({"keys": rad + ["SQRT"], "assert_rows": 2})
        else:
            # 11-2: square root of a division result.
            steps.append({"keys": int_entry(rng, rng.randint(3, 8)) + ["/"], "assert_rows": 1})
            steps.append({"keys": [str(rng.randint(2, 9)), "="], "assert_rows": 2})
            steps.append({"keys": ["SQRT"], "assert_rows": 2})
        return steps


class MemBlock:
    """Memory accumulation (manual 7-1, 7-2, 8-1, 9-1, 10-1; 5/4 rounding)."""

    def __init__(self, rng, dp, rounding, kind):
        self.dp, self.kind = dp, kind
        self.cap = max(2, 9 - dp)

    def factor(self, rng, limit):
        ia = rng.randint(1, max(1, min(limit, 4)))
        sf = rng.randint(0, min(self.dp, 3)) if self.dp > 0 else 0
        return entry(rng, ia, sf)

    def build(self, rng):
        steps = []
        if self.kind == "mem_eq":
            # 7-1/7-2: constant multiplicand/divisor with M=+ / M=- chains.
            steps.append({"keys": ["CM"], "assert_rows": 0})
            a = self.factor(rng, 3)
            op = "*" if rng.random() < 0.6 else "/"
            steps.append({"keys": a + [op], "assert_rows": 1})
            for _ in range(rng.randint(2, 3)):
                b = self.factor(rng, 4)
                sign = rng.choice(["M=+", "M=-"]) if self.dp > 0 else "M=+"
                steps.append({"keys": b + [sign], "assert_rows": 2})
            steps.append({"keys": ["CM"], "assert_rows": 1})
        elif self.kind == "mem_avg":
            # 10-1: memory-held count times a price, then the average.
            steps.append({"keys": ["CM"], "assert_rows": 0})
            steps.append({"keys": int_entry(rng, rng.randint(1, 3)) + ["M+"], "assert_rows": 1})
            price = self.factor(rng, 4)
            steps.append({"keys": price + ["*"], "assert_rows": 0})
            steps.append({"keys": [str(rng.randint(1, 9)), "="], "assert_rows": 2})
            steps.append({"keys": ["/"], "assert_rows": 1})
            steps.append({"keys": ["CM"], "assert_rows": 1})
            steps.append({"keys": ["="], "assert_rows": 2})
        elif self.kind == "mem_invoice":
            # 9-1 (trimmed): constant multiply into memory, recall, percent off.
            steps.append({"keys": ["CM"], "assert_rows": 0})
            steps.append({"keys": self.factor(rng, 2) + ["*"], "assert_rows": 1})
            steps.append({"keys": self.factor(rng, 4) + ["M=+"], "assert_rows": 2})
            steps.append({"keys": ["RM"], "assert_rows": 1})
            steps.append({"keys": ["*"], "assert_rows": 1})
            steps.append({"keys": [str(rng.randint(1, 20)), "%"], "assert_rows": 2})
            steps.append({"keys": ["M-"], "assert_rows": 1})
            steps.append({"keys": ["RM"], "assert_rows": 1})
            steps.append({"keys": ["CM"], "assert_rows": 1})
        return steps


class OverflowBlock:
    """Documented capacity limits (manual 14-1..14-3, 15-1..15-4, 16-1..16-4)."""

    def __init__(self, rng, dp, rounding, kind):
        self.dp, self.rounding, self.kind = dp, rounding, kind

    def build(self, rng):
        steps = []
        if self.kind == "add_entry":
            # 14-2: an integer entry whose DP-aligned width exceeds 14 digits.
            # Typed digits stay <= 14; the overflow comes from alignment.
            ia = (14 - self.dp) + rng.randint(1, 3)
            steps.append({"keys": int_entry(rng, ia) + ["+"], "assert_rows": 1})
        elif self.kind == "add_recover":
            # 14-3 (DP=8, IN): the accumulator overflows, CE recovers,
            # the surviving total is still printable. Both operands start
            # with 5..9 so their sum always crosses into a 7th digit.
            a = [str(rng.randint(5, 9))] + [str(rng.randint(0, 9)) for _ in range(5)]
            b = [str(rng.randint(5, 9))] + [str(rng.randint(0, 9)) for _ in range(5)]
            steps.append({"keys": a + ["+"], "assert_rows": 1})
            steps.append({"keys": b + ["+"], "assert_rows": 2})
            steps.append({"keys": ["CE", "+"], "assert_rows": 1})
            steps.append({"keys": ["="], "assert_rows": 1})
        elif self.kind == "mul":
            # 15-1..15-4: the product exceeds the printed-digit capacity.
            if self.dp == 0:
                a = int_entry(rng, 9)
                b = ["1", "0", "0", "0", "0", "0", "0"]
            else:
                a = int_entry(rng, 14 - self.dp + 1)
                b = ["1"]
            steps.append({"keys": a + ["*"], "assert_rows": 1})
            # Overflow suppresses the result print; assert only the final
            # dotted row, like manual 15-2.
            steps.append({"keys": b + ["="], "assert_rows": 1})
        elif self.kind == "div":
            # 16-1..16-4: the quotient exceeds the capacity.
            a = int_entry(rng, rng.randint(8, 10))
            if self.rounding == 0:
                tiny = [".", "0", "0", "0", "0", "0", "0", "3"]  # 0.0000003
            else:
                tiny = [str(rng.randint(2, 9))]
            steps.append({"keys": a + ["/"], "assert_rows": 1})
            steps.append({"keys": tiny + ["="], "assert_rows": 1})
        return steps


MENU = [
    ("add", 18),
    ("mul_simple", 5), ("mul_constant", 4), ("mul_powers", 3),
    ("mul_chain_st", 3), ("mul_correction", 2), ("mul_ex", 3),
    ("div_simple", 5), ("div_constant", 3), ("div_chain", 3),
    ("div_repeat_eq", 2), ("div_mem_dividend", 3),
    ("pct_mul", 3), ("pct_div", 3),
    ("sqrt_entry", 4), ("sqrt_result", 2),
    ("mem_eq", 6), ("mem_avg", 3), ("mem_invoice", 3),
    ("ovf_add_entry", 2), ("ovf_add_recover", 3), ("ovf_mul", 3), ("ovf_div", 3),
]


def eligible(kind, dp, rounding):
    if kind.startswith("mem_") or kind == "div_mem_dividend":
        return rounding == 1 and 0 <= dp <= 6
    if kind == "ovf_add_recover":
        return dp == 8 and rounding == 8
    if kind == "ovf_add_entry":
        return dp >= 2 and rounding != 0
    if kind == "ovf_mul":
        return dp in (0, 8)
    if kind == "ovf_div":
        return dp in (0, 8)
    if kind == "sqrt_entry":
        return rounding != 8
    if kind == "sqrt_result":
        # The ROM prints a recalled radicand as-is; the host core rescales it
        # to DP. They agree only when WR is already DP-scaled (11-2).
        return rounding == 1
    if kind.startswith("mul_") or kind.startswith("div_") or kind.startswith("pct_"):
        return rounding != 0  # FL result widths are not core-agreed yet
    return True


def want_lamps(step):
    keys = step["keys"]
    if not keys:
        return False
    if keys[-1] in MEMORY_KEYS or keys[-1] == "=":
        return True
    if "CE" in keys:
        return True
    return "lamps" in step


def make_example(rng, dp, rounding):
    names = [k for k, _ in MENU]
    weights = [w for _, w in MENU]

    kinds = [rng.choices(names, weights=weights)[0]]
    for _ in range(rng.choices([0, 1, 2], weights=[5, 3, 2])[0]):
        nxt = rng.choices(names, weights=weights)[0]
        if nxt.startswith("ovf") and any(k.startswith("ovf") for k in kinds):
            continue
        kinds.append(nxt)
    kinds = [k for k in kinds if eligible(k, dp, rounding)]
    if not kinds:
        kinds = ["add"]

    steps = []
    titles = []
    for bi, kind in enumerate(kinds):
        if steps:
            steps.append({"keys": ["C"], "assert_rows": 0})
        if kind == "add":
            has_eq = not (rounding == 0 and rng.random() < 0.7)
            block = AddBlock(rng, dp, rounding, with_total=has_eq,
                             allow_dp_change=bi == len(kinds) - 1)
            steps.extend(block.build(rng))
            titles.append("add/subtract chain")
        elif kind.startswith("mul_"):
            steps.extend(MulBlock(rng, dp, rounding, kind[4:]).build(rng))
            titles.append("multiplication " + kind[4:])
        elif kind.startswith("div_"):
            steps.extend(DivBlock(rng, dp, rounding, kind[4:]).build(rng))
            titles.append("division " + kind[4:])
        elif kind.startswith("pct_"):
            steps.extend(PercentBlock(rng, dp, rounding, kind[4:]).build(rng))
            titles.append("percentage " + kind[4:])
        elif kind.startswith("sqrt_"):
            steps.extend(SqrtBlock(rng, dp, rounding, kind[5:]).build(rng))
            titles.append("square root of " + kind[5:])
        elif kind.startswith("mem_"):
            steps.extend(MemBlock(rng, dp, rounding, kind[4:]).build(rng))
            titles.append("memory " + kind[4:])
        elif kind.startswith("ovf_"):
            steps.extend(OverflowBlock(rng, dp, rounding, kind[4:]).build(rng))
            titles.append("capacity overflow " + kind[4:])

    for st in steps:
        if st["assert_rows"] > 0 and want_lamps(st):
            st["lamps"] = True
        sw = st.get("switches", {})
        if "precision" in sw:
            assert sw["precision"] in (0, 1, 2, 3, 4, 5, 6, 8), sw
        if "rounding" in sw:
            assert sw["rounding"] in (0, 1, 8), sw
    return {"title": "; ".join(titles[:4]), "steps": steps, "dp": dp}


def programs(doc):
    return [
        {k: st[k] for k in ("keys", "assert_rows", "switches") if k in st}
        for ex in doc["examples"]
        for st in ex["steps"]
    ]


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--check", action="store_true",
                    help="compare regenerated programs with the committed JSON")
    args = ap.parse_args()

    manual = json.loads((ORACLE / "manual_examples.json").read_text(encoding="utf-8"))
    rng = random.Random(SEED)
    examples = []
    for i in range(1, COUNT + 1):
        dp = rng.choices([0, 1, 2, 3, 4, 6, 8], weights=[3, 2, 4, 2, 2, 2, 2])[0]
        rounding = rng.choices([0, 1, 8], weights=[1, 1, 1])[0]
        ex = make_example(rng, dp, rounding)
        examples.append({
            "id": f"R{i:04d}",
            "title": ex["title"],
            "switches": {"precision": dp, "rounding": rounding},
            "setup_keys": SETUP_KEYS,
            "steps": ex["steps"],
        })

    doc = {
        "schema_version": 1,
        "key_codes": manual["key_codes"],
        "source": {
            "title": "Seeded random calculator programs (Busicom 141-PF)",
            "generator": "platform/scripts/gen_random_tests.py",
            "seed": SEED,
            "count": COUNT,
            "expected_rows": (
                "Computed by the firmware host core via "
                "firmware/tests/random_calculator_tests.rs (dump_random_oracle), "
                "which independently reproduces oracle/manual_examples.json 42/42 "
                "and the full original-ROM port trace. Expected output is never "
                "derived from the emulator under test."
            ),
        },
        "coverage_notes": [
            "Program shapes are composed only from behaviour anchored by the 42 "
            "manual examples and the full-row port trace.",
            "assert_rows gives the number of trailing rows each step asserts; "
            "intermediate echo rows are deliberately unchecked (suffix rule).",
            "SIGN, the 00 key and fractional entries longer than DP are not "
            "oracle-anchored and are excluded from this corpus.",
            "Add/subtract chains ending in '=' never use rounding FL (no manual "
            "example anchors FL totals); memory blocks run at 5/4 as in their "
            "manual sources; overflow recovery runs at IN with DP=8 (14-3).",
        ],
        "examples": examples,
    }

    out = ORACLE / "random_calculator_tests.json"
    if args.check:
        committed = json.loads(out.read_text(encoding="utf-8"))
        if programs(committed) != programs(doc):
            sys.exit("ERROR: committed random corpus drifted from seed; regenerate")
        total = sum(len(ex["steps"]) for ex in examples)
        keys = sum(len(st.get("keys", ())) for ex in examples for st in ex["steps"])
        print(f"check OK: {COUNT} examples, {total} steps, {keys} key presses")
        return 0
    text = json.dumps(doc, indent=2, ensure_ascii=False) + "\n"
    out.write_text(text, encoding="utf-8", newline="\n")
    total = sum(len(ex["steps"]) for ex in examples)
    keys = sum(len(st.get("keys", ())) for ex in examples for st in ex["steps"])
    print(f"wrote {out} ({COUNT} examples, {total} steps, {keys} key presses)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
