#!/usr/bin/env python3
"""as8008.py - a small two-pass 8008 assembler for period sources.

Builds the oracle binaries of the Micral N titles from their original
listings (Shooting Stars as transcribed for the Macro Assembler AS), so the
original program can run on the simulators. Independent of llvm-mc: it is
written from the MCS-8 opcode table, like llvm-i8008/tests/mc/gen_encodings.py.

    as8008.py <source.asm> <out.bin> [--origin ADDR] [--listing FILE]

Accepted syntax (the subset these sources use):
  * labels `NAME:`, `NAME EQU expr`, `ORG expr`, `DB item, ...` (numbers,
    expressions, "strings", 'c' characters), `cpu`, `radix N`, `PAGE`,
    `END`, `;` comments;
  * numbers in the current radix (AS `radix 8` makes `100` octal), with
    suffixes B (binary), O/Q (octal), D (decimal), H (hex), and 'c';
  * operators + - * / & | and parentheses (left to right, * / & first);
  * the 1972 mnemonics (`LAB`, `LAM`, `LAI n`, `ADI n`, `JTZ addr`,
    `RST n`, `INP n`, `OUT n`, ...).
The output covers the lowest to the highest address written.
"""
import re
import sys

REG = "ABCDEHLM"
ALU = ["AD", "AC", "SU", "SB", "ND", "XR", "OR", "CP"]
COND = "CZSP"


def opcode(mn):
    """(opcode, operand kind) for a mnemonic; kind: None, 'imm', 'addr',
    'rst', 'inp', 'out'."""
    m = mn.upper()
    if len(m) == 3 and m[0] == "L":
        d, s = m[1], m[2]
        if d in REG and s in REG and not (d == "M" and s == "M"):
            return 0o300 | REG.index(d) << 3 | REG.index(s), None
        if d in REG and s == "I":
            return 0o006 | REG.index(d) << 3, "imm"
    if len(m) == 3 and m[:2] in ("IN", "DC") and m[2] in "BCDEHL":
        return (0o000 if m[:2] == "IN" else 0o001) | REG.index(m[2]) << 3, None
    if len(m) == 3 and m[:2] in ALU:
        o = ALU.index(m[:2])
        if m[2] in REG:
            return 0o200 | o << 3 | REG.index(m[2]), None
        if m[2] == "I":
            return 0o004 | o << 3, "imm"
    rot = {"RLC": 0o002, "RRC": 0o012, "RAL": 0o022, "RAR": 0o032}
    if m in rot:
        return rot[m], None
    if m == "JMP":
        return 0o104, "addr"
    if m == "CAL":
        return 0o106, "addr"
    if m == "RET":
        return 0o007, None
    if m == "HLT":
        return 0o377, None
    if len(m) == 3 and m[0] in "JCR" and m[1] in "FT" and m[2] in COND:
        c = COND.index(m[2])
        t = 1 if m[1] == "T" else 0
        if m[0] == "J":
            return 0o100 | t << 5 | c << 3, "addr"
        if m[0] == "C":
            return 0o102 | t << 5 | c << 3, "addr"
        return 0o003 | t << 5 | c << 3, None
    if m == "RST":
        return 0o005, "rst"
    if m == "INP":
        return 0o101, "inp"
    if m == "OUT":
        return 0o101, "out"
    return None, None


class Asm:
    def __init__(self):
        self.sym = {}
        self.radix = 10

    def number(self, tok):
        t = tok.upper()
        if t.startswith("0X"):
            return int(t[2:], 16)
        for suf, base in (("B", 2), ("O", 8), ("Q", 8), ("D", 10), ("H", 16)):
            if t.endswith(suf) and re.fullmatch(r"[0-9A-F]+", t[:-1]) and (
                    suf != "B" or re.fullmatch(r"[01]+", t[:-1])):
                if suf == "D" and self.radix == 16:
                    break
                return int(t[:-1], base)
        return int(t, self.radix)

    def value(self, expr, pc, need):
        toks = re.findall(r"'(?:[^']|'')'|\"[^\"]*\"|[A-Za-z_.$][A-Za-z0-9_.$]*|[0-9][0-9A-Za-z]*|[-+*/&|()]", expr)
        pos = 0

        def atom():
            nonlocal pos
            t = toks[pos]
            pos += 1
            if t == "(":
                v = binop()
                pos += 1  # ')'
                return v
            if t == "-":
                return -atom()
            if t.startswith("'"):
                return ord(t[1])
            if t == "$":
                return pc
            if re.match(r"[0-9]", t):
                return self.number(t)
            if t in self.sym:
                return self.sym[t]
            if need:
                raise SystemExit(f"as8008: undefined symbol {t}")
            return 0

        def binop():
            nonlocal pos
            v = atom()
            while pos < len(toks) and toks[pos] in "+-*/&|":
                op = toks[pos]
                pos += 1
                r = atom()
                v = {"+": v + r, "-": v - r, "*": v * r, "/": v // r if r else 0,
                     "&": v & r, "|": v | r}[op]
            return v

        return binop()

    def split_args(self, s):
        out, cur, q = [], "", None
        for ch in s:
            if q:
                cur += ch
                if ch == q:
                    q = None
            elif ch in "'\"":
                q = ch
                cur += ch
            elif ch == ",":
                out.append(cur.strip())
                cur = ""
            else:
                cur += ch
        if cur.strip():
            out.append(cur.strip())
        return out

    def run(self, lines, need):
        pc, mem, listing = 0, {}, []
        self.radix = 10
        for n, raw in enumerate(lines, 1):
            line = raw.split(";", 1)[0] if "'" not in raw and '"' not in raw else raw
            # strip a comment that is outside quotes
            q, cut = None, len(line)
            for i, ch in enumerate(line):
                if q:
                    if ch == q:
                        q = None
                elif ch in "'\"":
                    q = ch
                elif ch == ";":
                    cut = i
                    break
            line = line[:cut].rstrip()
            if not line.strip():
                continue
            m = re.match(r"\s*([A-Za-z_.$][A-Za-z0-9_.$]*)\s*:\s*(.*)$", line)
            label = None
            if m:
                label, line = m.group(1), m.group(2)
            f = line.split(None, 1)
            if len(f) >= 2 and f[1].upper().startswith("EQU"):
                self.sym[f[0]] = self.value(f[1][3:], pc, need)
                continue
            if label:
                self.sym[label] = pc
            if not f:
                continue
            op, arg = f[0], (f[1] if len(f) > 1 else "")
            u = op.upper()
            if u in ("CPU", "PAGE", "END", "TITLE"):
                continue
            if u == "RADIX":
                self.radix = int(arg.strip())
                continue
            if u == "ORG":
                pc = self.value(arg, pc, need)
                continue
            if u in ("DB", "DATA"):
                for a in self.split_args(arg):
                    if a.startswith('"'):
                        for ch in a[1:-1]:
                            mem[pc] = ord(ch)
                            pc += 1
                    else:
                        mem[pc] = self.value(a, pc, need) & 0xFF
                        pc += 1
                continue
            code, kind = opcode(u)
            if code is None:
                raise SystemExit(f"as8008: line {n}: unknown mnemonic {op}")
            start = pc
            if kind == "rst":
                code |= (self.value(arg, pc, need) & 7) << 3
            elif kind == "inp":
                code |= (self.value(arg, pc, need) & 7) << 1
            elif kind == "out":
                code |= (self.value(arg, pc, need) & 31) << 1
            mem[pc] = code
            pc += 1
            if kind == "imm":
                mem[pc] = self.value(arg, pc, need) & 0xFF
                pc += 1
            elif kind == "addr":
                a = self.value(arg, pc, need) & 0x3FFF
                mem[pc], mem[pc + 1] = a & 0xFF, a >> 8
                pc += 2
            listing.append(f"{start:04X}  {' '.join('%02X' % mem[i] for i in range(start, pc)):<9} {raw.rstrip()}")
        return mem, listing


def main():
    import argparse
    ap = argparse.ArgumentParser()
    ap.add_argument("src")
    ap.add_argument("out")
    ap.add_argument("--origin", type=lambda s: int(s, 0))
    ap.add_argument("--listing")
    a = ap.parse_args()
    lines = open(a.src, encoding="latin-1").read().splitlines()
    asm = Asm()
    asm.run(lines, need=False)
    mem, listing = asm.run(lines, need=True)
    lo = a.origin if a.origin is not None else min(mem)
    hi = max(mem) + 1
    img = bytes(mem.get(i, 0) for i in range(lo, hi))
    open(a.out, "wb").write(img)
    if a.listing:
        open(a.listing, "w").write("\n".join(listing) + "\n")
    print(f"as8008: {a.out}: {len(img)} B at 0x{lo:04X}")


if __name__ == "__main__":
    main()
