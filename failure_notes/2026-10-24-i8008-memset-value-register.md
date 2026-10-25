# memset read its fill value from the wrong register

- Area: llvm-i8008 (`rt/mem.s`, `scripts/exec_tests.py`, `spec/abi.md` §6)
- Found by: the SCELBAL oracle. The Rust SCELBAL passed all ten transcripts
  on the host and failed four on the RTL: a false comparison printed
  `0.5965428E+29`, `IF I=2` with I = 1 was taken, `INPUT C$` stored
  5395165 for `Z`.

## What broke

`cfalse` (FPACC = 0) clears five consecutive bytes, which LLVM turned into
`memset(p, 0, 5)`. llc passes the value as an i8: the pointer takes BC and
the value the next free 8-bit register, D. `rt/mem.s` stored E, so memset
filled the bytes with whatever E held. Every FALSE and every cleared
accumulator in SCELBAL got garbage.

## Root cause and why our tests missed it

`abi.md` said `memset(dst BC, c DE, n)`, as if the value were C's `int`;
SelectionDAG gives the libcall the IR value's own type. The routine was
written from that line, and the execution test (`exec_tests.py`) was
written from the routine: it called memset with the value in E, so it
checked the routine against its own contract, not against what llc emits.
No compiled-IR test called memset, and the earlier titles never memset a
nonzero length with a live E.

## Fix and re-verification

- `rt/mem.s` stores D; its header and `abi.md` §6 give each helper's
  registers and say why memset's value is in D.
- `exec_tests.py` puts the value in D and junk in E (4650/4650 pass).
- `tests/codegen/mem.ll` `fill5` checks that llc puts memset's value in D,
  so the contract is pinned from the compiler's side.
- SCELBAL: 10/10 on the RTL, loaded directly and booted.

## Prevention

A runtime helper's argument registers come from what llc emits for a call
to it (a lit test of the call), not from its C prototype; its execution
test uses those registers and fills the others with junk.
