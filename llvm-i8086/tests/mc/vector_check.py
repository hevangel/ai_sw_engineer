#!/usr/bin/env python3
"""Check the i8086 MC layer against the physical-chip instruction vectors.

The SingleStepTests/8086 corpus (captured from an Intel P80C86A-2; the hardware
project caches it) lists, for thousands of random instruction instances, the
exact machine bytes and the disassembly MartyPC gave them. That makes it an
independent opcode table for this backend, which neither wrote:

  1. disassembly:  llvm-mc --disassemble must decode every instance, with the
                   same mnemonic as the corpus and nothing left over;
  2. round trip:   the printed text must assemble back, and the bytes must be
                   the original bytes, or the text must disassemble identically
                   from the re-assembled bytes (an alternative encoding of the
                   same instruction, e.g. 01 D8 vs 03 C3).

Relative branches are checked for decoding and mnemonic only (their text holds
an absolute address). Instructions this backend does not implement (ESC,
undocumented encodings) are skipped, as in the RTL's own selection.

    vector_check.py <llvm-bin> <vector_dir> [cases_per_opcode_file] [seed]
"""
import gzip
import json
import os
import random
import re
import subprocess
import sys

PREFIXES = {0x26, 0x2E, 0x36, 0x3E, 0xF0, 0xF2, 0xF3}
SENTINEL = ['hlt', 'hlt', 'hlt']
CANON = {
    'jz': 'je', 'jnz': 'jne', 'jc': 'jb', 'jnae': 'jb', 'jnc': 'jae', 'jnb': 'jae',
    'jna': 'jbe', 'jnbe': 'ja', 'jpe': 'jp', 'jpo': 'jnp', 'jnge': 'jl', 'jnl': 'jge',
    'jng': 'jle', 'jnle': 'jg', 'retn': 'ret', 'jmpf': 'jmp', 'callf': 'call',
    'repe': 'rep', 'repz': 'rep', 'repnz': 'repne', 'sal': 'shl', 'xlatb': 'xlat',
    'loopz': 'loope', 'loopnz': 'loopne',
}
BRANCHES = {'jmp', 'call', 'jcxz', 'loop', 'loope', 'loopne'} | {
    'jo', 'jno', 'jb', 'jae', 'je', 'jne', 'jbe', 'ja', 'js', 'jns', 'jp', 'jnp',
    'jl', 'jge', 'jle', 'jg'}


def canon(m):
    return CANON.get(m, m)


def run(tool, args, text):
    p = subprocess.run([tool] + args, input=text, capture_output=True, text=True)
    return p.stdout, p.stderr


def split_cases(lines, n):
    """Split output lines at each run of three hlt; expects exactly n groups."""
    groups, cur, i = [], [], 0
    while i < len(lines):
        if lines[i:i + 3] == SENTINEL:
            groups.append(cur)
            cur = []
            i += 3
        else:
            cur.append(lines[i])
            i += 1
    return groups if len(groups) == n and not cur else None


def disassemble(llvm, cases):
    """Per case: the list of instruction texts (None if the batch desynchronised)."""
    text = ''.join(' '.join(f'0x{b:02x}' for b in c + [0xF4] * 3) + '\n' for c in cases)
    out, err = run(llvm + '/llvm-mc', ['--disassemble', '-triple=i8086'], text)
    lines = [l.strip() for l in out.splitlines() if l.strip()]
    groups = split_cases(lines, len(cases))
    if groups is None:
        return None
    # A warning means bytes the decoder could not make an instruction of.
    for m in re.finditer(r'<stdin>:(\d+):\d+: warning: invalid instruction encoding', err):
        groups[int(m.group(1)) - 1] = None
    return groups


def assemble(llvm, groups):
    """Per group: the bytes of its instructions, or None if a fixup is involved."""
    text = ''
    for g in groups:
        text += '\n'.join(g) + '\nhlt\nhlt\nhlt\n'
    out, err = run(llvm + '/llvm-mc', ['-triple=i8086', '-show-encoding'], text)
    results, cur, fixup, i = [], [], False, 0
    enc = re.compile(r'encoding: \[(.*)\]')
    lines = out.splitlines()
    hlts = 0
    for l in lines:
        m = enc.search(l)
        if 'fixup' in l and not m:
            fixup = True
            continue
        if not m:
            continue
        b = [int(x, 16) if x.startswith('0x') else None for x in m.group(1).split(',')]
        if l.strip().startswith('hlt'):
            hlts += 1
            if hlts == 3:
                results.append(None if fixup else cur)
                cur, fixup, hlts = [], False, 0
            continue
        hlts = 0
        if None in b:
            fixup = True
        else:
            cur += b
    return results if len(results) == len(groups) else None


def load_cases(vdir, per, seed):
    meta = json.load(open(os.path.join(vdir, 'metadata.json')))['opcodes']
    rng = random.Random(seed)
    cases = []
    for name in sorted(os.listdir(vdir)):
        if not name.endswith('.json.gz'):
            continue
        tests = json.load(gzip.open(os.path.join(vdir, name)))
        rng.shuffle(tests)
        taken = 0
        for t in tests:
            code = t['bytes']
            pos = 0
            while pos < len(code) and code[pos] in PREFIXES:
                pos += 1
            if pos == len(code):
                continue
            op = code[pos]
            info = meta[f'{op:02X}']
            if 'reg' in info:
                info = {**info, **info['reg'].get(str((code[pos + 1] >> 3) & 7), {})}
            if info.get('status') != 'normal' or op == 0x0F:
                continue
            if op in (0x8C, 0x8E) and (((code[pos + 1] >> 3) & 7) > 3 or (op == 0x8E and (code[pos + 1] >> 3) & 7 == 1)):
                continue
            if op in (0x8D, 0xC4, 0xC5) and code[pos + 1] >> 6 == 3:
                continue
            if op in (0xFF,) and (code[pos + 1] >> 3) & 7 in (3, 5) and code[pos + 1] >> 6 == 3:
                continue
            cases.append((t['name'], code))
            taken += 1
            if taken >= per:
                break
    return cases


def main():
    llvm, vdir = sys.argv[1], sys.argv[2]
    per = int(sys.argv[3]) if len(sys.argv) > 3 else 60
    seed = int(sys.argv[4]) if len(sys.argv) > 4 else 1
    cases = load_cases(vdir, per, seed)
    print(f'{len(cases)} instruction instances from the corpus')
    stats = dict(decoded=0, undecoded=0, wrong_mnemonic=0, roundtrip_same=0,
                 roundtrip_alt=0, roundtrip_bad=0, branch_skipped=0, desync=0)
    problems = []
    BATCH = 50
    for start in range(0, len(cases), BATCH):
        batch = cases[start:start + BATCH]
        dis = disassemble(llvm, [c for _, c in batch])
        if dis is None:
            # A decode failure ate the sentinel: redo one at a time.
            stats['desync'] += 1
            dis = []
            for name, c in batch:
                one = disassemble(llvm, [c])
                dis.append(one[0] if one else None)
        groups, who = [], []
        for (name, code), texts in zip(batch, dis):
            want = canon(re.sub(r'^(?:(?:cs|ds|es|ss|rep|repe|repne|repnz|repz|lock)\s+)+', '', name).split()[0])
            if not texts:
                stats['undecoded'] += 1
                problems.append(('undecoded', name, code, texts))
                continue
            mnems = [canon(t.split()[0]) for t in texts]
            if want not in mnems:
                stats['wrong_mnemonic'] += 1
                problems.append(('mnemonic', name, code, texts))
                continue
            stats['decoded'] += 1
            if want in BRANCHES:
                stats['branch_skipped'] += 1
                continue
            groups.append(texts)
            who.append((name, code, texts))
        if not groups:
            continue
        enc = assemble(llvm, groups)
        if enc is None:
            for name, code, texts in who:
                one = assemble(llvm, [texts])
                e = one[0] if one else None
                classify(stats, problems, llvm, name, code, texts, e)
        else:
            for (name, code, texts), e in zip(who, enc):
                classify(stats, problems, llvm, name, code, texts, e)
    print(json.dumps(stats))
    for kind, name, code, extra in problems[:25]:
        print(f'  {kind}: {name!r} {bytes(code).hex()} -> {extra}')
    bad = stats['undecoded'] + stats['wrong_mnemonic'] + stats['roundtrip_bad']
    print('PASS' if bad == 0 else f'FAIL: {bad} problems')
    sys.exit(1 if bad else 0)


def classify(stats, problems, llvm, name, code, texts, e):
    if e is None:
        problems.append(('did not assemble', name, code, texts))
        stats['roundtrip_bad'] += 1
    elif e == code:
        stats['roundtrip_same'] += 1
    else:
        again = disassemble(llvm, [e])
        if again and again[0] == texts:
            stats['roundtrip_alt'] += 1
        else:
            stats['roundtrip_bad'] += 1
            problems.append(('round trip', name, code, (texts, bytes(e).hex(), again)))


if __name__ == '__main__':
    main()
