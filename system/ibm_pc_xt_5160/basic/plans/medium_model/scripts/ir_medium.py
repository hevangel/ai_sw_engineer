#!/usr/bin/env python3
"""Medium-model IR pass for llvm-i8086 (proof of concept).

    ir_medium.py <in.ll> <out.ll> [--far-intrinsics] [--medium SIZES|-] [--seg0 NAME,...]
                 [--cap0 BYTES] [--cap BYTES] [--near NAME,...]

--far-intrinsics: calls to `__far_r8(seg, off)`, `__far_r16`, `__far_w8(seg, off, v)`,
  `__far_w16` become loads and stores through `ptr addrspace(1)` (abi.md D7: offset in the
  low word, segment in the high word), which the backend's I8086FarPointers pass expands
  inline (push es / mov es / es: access / pop es). Rust cannot name address space 1; this is
  how it reaches a far data segment without a call per byte.
--medium SIZES: every defined function, and every declared one not in --near and not an LLVM
  intrinsic, gets calling convention `cc 1000` (abi.md D8: far call, retf); so does every
  direct call to one. Functions are packed into code segments: segment 0 (sections .text.*)
  holds --seg0 names and as much as --cap0 allows, the rest go first-fit-decreasing into
  segments 1.. of --cap bytes, as `section ".far.text.<n>.<k>"`. SIZES is a file of
  `<size> <name>` lines from a near-model compile (or - to put everything in segment 0).
  An indirect call or a function whose address is taken is refused (16-bit function pointers
  cannot cross segments; see the report for the thunk-table extension).
Writes the assignment to <out.ll>.segments (name, segment, size).
"""
import re
import sys

args = sys.argv[1:]
src, dst = args[0], args[1]
opts = {}
i = 2
while i < len(args):
    a = args[i]
    if a in ('--far-intrinsics',):
        opts[a] = True
        i += 1
    else:
        opts[a] = args[i + 1]
        i += 2

NAME = r'(?:"[^"]+"|[\w.$]+)'
lines = open(src, encoding='utf-8').read().split('\n')

# ---- far intrinsics -------------------------------------------------------------------
FAR = {'__far_r8': ('load', 'i8'), '__far_r16': ('load', 'i16'),
       '__far_w8': ('store', 'i8'), '__far_w16': ('store', 'i16')}
ARG = r'(?:i16|i8)(?: (?:noundef|zeroext|signext))* ([%\w.\-]+)'
tmp = [0]


def fresh():
    tmp[0] += 1
    return '%%__farp%d' % tmp[0]


def far_ptr(seg, off, out, indent):
    a, b, c, d, p = fresh(), fresh(), fresh(), fresh(), fresh()
    out += [f'{indent}{a} = zext i16 {seg} to i32', f'{indent}{b} = shl i32 {a}, 16',
            f'{indent}{c} = zext i16 {off} to i32', f'{indent}{d} = or i32 {b}, {c}',
            f'{indent}{p} = inttoptr i32 {d} to ptr addrspace(1)']
    return p


if '--far-intrinsics' in opts:
    out = []
    for ln in lines:
        m = re.match(r'^(\s*)(?:(%[\w.\-]+) = )?(?:tail |notail )?call [^@]*@(__far_[rw](?:8|16))\((.*)\)(.*)$', ln)
        if m:
            indent, res, fn, argstr = m.group(1), m.group(2), m.group(3), m.group(4)
            vals = re.findall(ARG, argstr)
            kind, ty = FAR[fn]
            p = far_ptr(vals[0], vals[1], out, indent)
            if kind == 'load':
                out.append(f'{indent}{res} = load {ty}, ptr addrspace(1) {p}, align 1')
            else:
                out.append(f'{indent}store {ty} {vals[2]}, ptr addrspace(1) {p}, align 1')
            continue
        if re.match(r'^declare .*@__far_[rw](?:8|16)\(', ln):
            continue
        out.append(ln)
    lines = out

# ---- medium model ---------------------------------------------------------------------
if '--medium' in opts:
    near = set(filter(None, opts.get('--near', '').split(',')))
    seg0 = set(filter(None, opts.get('--seg0', 'main').split(',')))
    defined, declared = [], set()
    for ln in lines:
        m = re.match(r'^define [^@]*@(' + NAME + r')\(', ln)
        if m:
            defined.append(m.group(1))
        m = re.match(r'^declare [^@]*@(' + NAME + r')\(', ln)
        if m and not m.group(1).startswith('llvm.'):
            declared.add(m.group(1))
    far = (set(defined) | declared) - near
    # sizes and packing
    sizes = {}
    if opts['--medium'] != '-':
        for ln in open(opts['--medium']):
            s, n = ln.split()
            sizes[n] = max(sizes.get(n, 0), int(s))
    cap0 = int(opts.get('--cap0', '0'), 0)
    cap = int(opts.get('--cap', '0xF000'), 0)
    seg = {}
    used = [0]
    for n in defined:
        if n in seg0:
            seg[n] = 0
            used[0] += sizes.get(n, 0)
    rest = sorted((n for n in defined if n not in seg), key=lambda n: -sizes.get(n, 0))
    if opts['--medium'] == '-':
        for n in rest:
            seg[n] = 0
    else:
        for n in rest:
            sz = sizes.get(n, 0) + 16  # far calls are 2 bytes longer
            for k in range(len(used)):
                c = cap0 if k == 0 else cap
                if used[k] + sz <= c:
                    seg[n] = k
                    used[k] += sz
                    break
            else:
                used.append(sz)
                seg[n] = len(used) - 1
    CC = re.compile(r'\b(fastcc|coldcc|ccc|cc \d+) ')
    LINK = {'internal', 'private', 'external', 'weak', 'linkonce_odr', 'weak_odr', 'linkonce',
            'dso_local', 'dso_preemptable', 'hidden', 'protected', 'default', 'available_externally'}

    def set_cc(head):  # head: text from 'define '/'declare ' up to '@'
        if CC.search(head):
            return CC.sub('cc 1000 ', head, count=1)
        words = head.split(' ')
        k = 1
        while k < len(words) and words[k] in LINK:
            k += 1
        return ' '.join(words[:k] + ['cc 1000'] + words[k:])

    bad = []
    out = []
    for ln in lines:
        m = re.match(r'^((?:define|declare) [^@]*)@(' + NAME + r')(\(.*)$', ln)
        if m and m.group(2) in far:
            head, name, tail = m.groups()
            ln = set_cc(head) + '@' + name + tail
            if ln.startswith('define') and seg.get(name, 0) > 0:
                sec = ' section ".far.text.%d.%s"' % (seg[name], name.strip('"'))
                j = ln.find(' !')
                j = ln.rfind(' {') if j < 0 else j
                ln = ln[:j] + sec + ln[j:]
            out.append(ln)
            continue
        # calls
        def fix_call(mc):
            pre, cc, rest, callee = mc.group(1), mc.group(2), mc.group(3), mc.group(4)
            if callee not in far:
                return mc.group(0)
            return pre + 'call cc 1000 ' + rest + '@' + callee + '('
        ln2 = re.sub(r'\b((?:tail |musttail |notail )?)call ((?:fastcc|coldcc|ccc|cc \d+) )?([^@%(]*?)@(' + NAME + r')\(',
                     fix_call, ln)
        if re.search(r'\bcall [^@(]*%[\w.\-]+\(', ln) and 'asm' not in ln:
            bad.append('indirect call: ' + ln.strip()[:120])
        # address taken: a function name used other than as a callee
        for r in re.finditer(r'@(' + NAME + r')', ln2):
            n = r.group(1)
            if n in far and not ln2.startswith(('define', 'declare')):
                after = ln2[r.end():r.end() + 1]
                if after != '(':
                    bad.append('address of %s taken: %s' % (n, ln2.strip()[:120]))
        out.append(ln2)
    if bad:
        sys.stderr.write('ir_medium: not supported in the medium model:\n  ' + '\n  '.join(bad[:20]) + '\n')
        sys.exit(1)
    lines = out
    with open(dst + '.segments', 'w') as f:
        for n in defined:
            f.write('%s %d %d\n' % (n, seg[n], sizes.get(n, 0)))
        f.write('# used: %s\n' % used)

open(dst, 'w', encoding='utf-8').write('\n'.join(lines))
