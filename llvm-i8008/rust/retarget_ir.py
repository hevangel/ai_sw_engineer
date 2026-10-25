#!/usr/bin/env python3
"""Retarget an AVR LLVM-IR module to the i8008 backend.

The pinned rustc's LLVM has no i8008 target, so we compile the firmware for
`avr-none` (the closest existing target: little-endian, 16-bit pointers at
8-bit alignment, n8 — see llvm-i8008/rust/README.md for the mapping) and then
rewrite the module header to i8008's triple and data layout.

This is a TEXTUAL rewrite of a .ll file, deliberately conservative: it rewrites
only what it can prove safe, and FAILS (nonzero exit, no output) on anything
target-specific it cannot map, so a silently-wrong module never reaches llc.

What it maps:
  * `target triple`      -> i8008-none
  * `target datalayout`  -> the exact i8008 string from spec/abi.md
  * `addrspace(1)` on CODE (define/declare/call/invoke) -> dropped. On AVR all
    code lives in program space (addrspace 1, Harvard); on i8008 code also
    lives in ROM but the datalayout's default/code space is 0. No DATA pointer
    in the firmware IR is typed `ptr addrspace(1)` (verified by the guard
    below), so dropping the code-space annotation is behaviour-preserving.

What it REFUSES (the module is target-specific in a way we must not fake):
  * an `avr`/`__flash`/`progmem` token, or a named-address-space DATA pointer
    (`ptr addrspace(N)` for N in {1,2,...} appearing as a value/alloca type)
  * a float/double type (the 8008 has no FP; the firmware must not emit any)
  * an i128 (the backend cannot legalize 128-bit on a 8-bit machine)
  * an AVR-specific intrinsic (llvm.avr.*)

i32/i64 are NOT refused here: they are legal IR that the backend must widen-
legalize (or that `opt -Oz` + libcalls lower). They ARE reported, because they
are the costly part of a 8-bit target and belong in the codegen work list.
"""
from __future__ import annotations

import re
import sys

I8008_TRIPLE = "i8008-none"
# Must match llvm-i8008/spec/abi.md §1 and rust/i8008-none.json character-for-character.
I8008_DATALAYOUT = "e-m:e-p:16:8-i8:8-i16:8-i32:8-i64:8-n8-a:8"


# A type keyword, not part of a value name such as %_0.i60.i128.i (LLVM names
# may contain ".", "$" and "-").
TYPE = r"(?<![\w.$%@-]){}(?![\w.$-])"


def fail(msg: str) -> "NoReturn":  # type: ignore[name-defined]
    sys.stderr.write(f"retarget_ir: REFUSED: {msg}\n")
    sys.exit(2)


def main() -> int:
    if len(sys.argv) != 3:
        sys.stderr.write("usage: retarget_ir.py <in.ll> <out.ll>\n")
        return 2
    src = open(sys.argv[1], encoding="utf-8").read()

    # --- hard refusals: target-specific constructs we must not fake ----------
    for tok in ("llvm.avr.", "__flash", "progmem", "addrspacecast"):
        if tok in src:
            fail(f"module contains {tok!r} (target-specific; cannot map to i8008)")
    if re.search(TYPE.format("i128"), src):
        fail("module uses i128 (not legalizable on a 8-bit target)")
    if re.search(TYPE.format("(float|double|fp128|half)"), src):
        fail("module uses floating point (the 8008 has no FP unit)")

    # A DATA pointer in a non-default address space would be a real memory-model
    # difference. Code carries `addrspace(1)` on AVR (handled below); a DATA ptr
    # would appear as a *typed value* `ptr addrspace(N)` used by load/store/
    # getelementptr/alloca. Detect that specific shape and refuse it.
    # A code address taken as an integer (`ptrtoint (ptr addrspace(1) @f to
    # i8)`, e.g. stored as the bytes of a patched CAL) is not a data pointer.
    code_ints = re.sub(r"ptrtoint \(ptr addrspace\(1\) @[^ ]+ to i\d+\)", "CODEINT", src)
    data_as = re.findall(
        r"(?:load|store|getelementptr[^\n]*?|alloca)[^\n]*\bptr addrspace\((\d+)\)", code_ints
    )
    bad = sorted({n for n in data_as if n != "0"})
    if bad:
        fail(f"data pointer(s) in address space {bad} (i8008 data space is 0)")

    # --- header rewrite -------------------------------------------------------
    src, n_triple = re.subn(
        r'^target triple = ".*"$', f'target triple = "{I8008_TRIPLE}"', src, flags=re.M
    )
    src, n_dl = re.subn(
        r'^target datalayout = ".*"$',
        f'target datalayout = "{I8008_DATALAYOUT}"',
        src,
        flags=re.M,
    )
    if n_triple != 1 or n_dl != 1:
        fail(f"expected exactly one triple and one datalayout (got {n_triple}, {n_dl})")

    # --- drop the code address-space annotation (Harvard program space) -------
    # `addrspace(1)` appears on: define/declare headers, call/invoke, and the
    # intrinsic `ptr` suffixes (`@llvm.memset.p0.i64 ... addrspace(1)`). Every
    # occurrence annotates CODE, not a data object (the data-ptr guard above
    # already proved no data pointer lives in a named space). Strip it.
    n_code_as = len(re.findall(r"\baddrspace\(1\)", src))
    src = re.sub(r"\s*addrspace\(1\)", "", src)
    # Defensive: no addrspace(>0) must survive.
    leftover = re.findall(r"addrspace\((\d+)\)", src)
    leftover = sorted({n for n in leftover if n != "0"})
    if leftover:
        fail(f"unexpected surviving addrspace {leftover} after rewrite")

    open(sys.argv[2], "w", encoding="utf-8").write(src)

    # --- report (stderr) ------------------------------------------------------
    i32 = len(re.findall(r"\bi32\b", src))
    i64 = len(re.findall(r"\bi64\b", src))
    sys.stderr.write(
        "retarget_ir: OK  "
        f"triple+datalayout->i8008, dropped {n_code_as} code addrspace(1); "
        f"i32 uses={i32}, i64 uses={i64} (reported, not refused)\n"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
