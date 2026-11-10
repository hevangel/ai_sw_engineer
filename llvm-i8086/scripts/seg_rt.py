#!/usr/bin/env python3
"""Medium model, libcalls (spec/abi.md D10; driven by build_rust_medium.sh).
llc calls the runtime library (rt/) with near
calls whatever segment the caller is in, so every code segment N >= 1 gets its own copy of
the routines it calls, renamed <sym>__sN and placed in sections .far.text.N.rt.<sym>.

    seg_rt.py <prog.s> <out.s> <rt_dir> <rt .s file>...
writes <out.s> (the program with the calls in segment N renamed) and <rt_dir>/rt_sN.s.
"""
import re, sys
prog, outp, rtdir, *rts = sys.argv[1:]
# the symbols the runtime defines
defs = set()
rtsrc = []
for f in rts:
    t = open(f).read()
    rtsrc.append(t)
    defs |= set(re.findall(r'^\s*\.globl\s+([\w.$]+)', t, re.M))
seg = 0
used = {}
out = []
for ln in open(prog).read().split('\n'):
    m = re.match(r'\s*\.section\s+([^,\s]+)', ln)
    if m:
        s = re.match(r'\.far\.text\.(\d+)\.', m.group(1))
        seg = int(s.group(1)) if s else 0
    if seg:
        def ren(mm):
            used.setdefault(seg, set()).add(mm.group(2))
            return mm.group(1) + mm.group(2) + '__s%d' % seg
        ln = re.sub(r'^(\s*(?:call|jmp)\s+)([\w.$]+)$', lambda mm: ren(mm) if mm.group(2) in defs else mm.group(0), ln)
    out.append(ln)
open(outp, 'w').write('\n'.join(out))
for n in sorted(used):
    body = []
    for t in rtsrc:
        t = re.sub(r'\.section\s+\.text\.([\w.$]+)', lambda mm: '.section .far.text.%d.rt.%s' % (n, mm.group(1)), t)
        t = re.sub(r'(?<![\w.$])(' + '|'.join(map(re.escape, sorted(defs, key=len, reverse=True))) + r')(?![\w$])',
                   lambda mm: mm.group(1) + '__s%d' % n, t)
        body.append(t)
    open('%s/rt_s%d.s' % (rtdir, n), 'w').write('\n'.join(body))
    print('segment %d: own copies of %s' % (n, ' '.join(sorted(used[n]))))
