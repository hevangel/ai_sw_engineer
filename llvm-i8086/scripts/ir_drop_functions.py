#!/usr/bin/env python3
"""Drop from an LLVM IR module the function definitions the i8086 backend cannot
compile or nobody needs: libm (compiler_builtins::math) and anything that mentions
the types bfloat, fp128 or x86_fp80 (there is no soft-float for them here),
and, repeatedly, every function that calls a dropped one.

    ir_drop_functions.py <in.ll> <out.ll>
"""
import re
import sys

BAD_TYPE = re.compile(r'\b(bfloat|fp128|x86_fp80|ppc_fp128)\b')
NAME = re.compile(r'^define [^@]*@("[^"]+"|[\w.$]+)\(')
REF = re.compile(r'@("[^"]+"|[\w.$]+)')
src, dst = sys.argv[1:3]

items = []  # ('text', line) | ('func', name, lines) | ('alias', name, target, line)
block, inside = [], False
for line in open(src, encoding='utf-8'):
    if not inside and line.startswith('define '):
        inside, block = True, [line]
        continue
    if inside:
        block.append(line)
        if line.startswith('}'):
            name = NAME.match(block[0]).group(1)
            items.append(('func', name, block))
            inside, block = False, []
        continue
    m = re.match(r'^@("[^"]+"|[\w.$]+) = .*\balias\b', line)
    if m:
        items.append(('alias', m.group(1), line))
    else:
        items.append(('text', line))

dropped = set()
for it in items:
    if it[0] == 'func' and ('4math' in it[1] or BAD_TYPE.search(''.join(it[2]))):
        dropped.add(it[1])
changed = True
while changed:
    changed = False
    for it in items:
        if it[0] == 'func' and it[1] not in dropped:
            refs = {r for r in REF.findall(''.join(it[2][1:]))}
            if refs & dropped:
                dropped.add(it[1])
                changed = True
        elif it[0] == 'alias' and it[1] not in dropped:
            if set(REF.findall(it[2].split('=', 1)[1])) & dropped:
                dropped.add(it[1])
                changed = True
out = []
for it in items:
    if it[0] == 'text':
        line = it[1]
        if line.startswith('!') and '@' in line:
            # metadata that points at a dropped function (call-graph hints): null instead
            for n in dropped:
                line = line.replace('ptr @' + n + '}', 'ptr null}').replace('ptr @' + n + ',', 'ptr null,')
        out.append(line)
    elif it[0] == 'func':
        if it[1] not in dropped:
            out.extend(it[2])
    elif it[1] not in dropped:
        out.append(it[2])
open(dst, 'w', encoding='utf-8').write(''.join(out))
print(f'dropped {len(dropped)} definitions and aliases', file=sys.stderr)
