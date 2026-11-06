#!/usr/bin/env python3
"""compare_screen.py <screen.txt> <expected> [<after command>]

<screen.txt> is martypc_oracle's output (one '|row' per line). Without <after command>,
<expected> lists rows ('#' lines are comments) that must appear in this order as consecutive
screen rows. With it, <expected> is an RTL transcript of the same DOS program: the rows the RTL
showed between the row ending with 'A><after command>' and the next 'A>' prompt must be the rows
MartyPC shows after its own 'A><after command>' row. Exit 0 if they match."""
import sys


def rows(path, transcript=False):
    out = []
    for line in open(path, encoding='latin-1'):
        line = line.rstrip('\n')
        if transcript:
            if line.startswith('|'):
                out.append(line[1:].rstrip())
        elif not line.startswith('#'):
            out.append(line[1:].rstrip() if line.startswith('|') else line.rstrip())
    return out


def after(rs, command):
    """The rows after the last row 'A><command>' up to the next prompt."""
    starts = [i for i, r in enumerate(rs) if r.upper().endswith('A>' + command.upper())]
    if not starts:
        return None
    body = []
    for r in rs[starts[-1] + 1:]:
        if r.startswith('A>'):
            break
        body.append(r)
    while body and not body[-1]:
        body.pop()
    return body


screen = rows(sys.argv[1], transcript=True)
if len(sys.argv) > 3:
    want = after(rows(sys.argv[2], transcript=True), sys.argv[3])
    got = after(screen, sys.argv[3])
else:
    want = [r for r in rows(sys.argv[2]) if True]
    while want and not want[-1]:
        want.pop()
    got = None
    for i in range(len(screen)):
        if screen[i:i + len(want)] == want:
            got = want
            break
    if got is None:
        got = screen
ok = got == want
print(('match: ' if ok else 'DIFFERENT:\n  expected: ') + repr(want) + ('' if ok else '\n  screen:   ' + repr(got)))
sys.exit(0 if ok else 1)
