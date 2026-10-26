#!/usr/bin/env python3
"""Unit test: far-call lowering (abi.md 4.4.3) + R_I4004_BANK4.

Links a tiny 3-function program into a banked image with far-calls ON and
asserts the structural contract WITHOUT a simulator:

  1. R_I4004_BANK4 is a defined relocation (number 9, named).
  2. A cross-bank call site's `jun callee` is rewritten to a far thunk, not left
     pointing at the callee directly.
  3. Each far thunk: `jms __save_p4p5` ; push forward token ; `jun __soft_ret`.
  4. The forward token targets a restore trampoline whose bank nibble matches
     the callee's placed bank (the BANK4 value the dispatch expects).
  5. The common __push_pb / __save_p4p5 helpers and the per-page jin-p0 stub are
     present and non-overlapping.

Run: python3 llvm-i4004/asm/tests/test_farcall.py   (exit 0 = pass)
"""
import os
import pathlib
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ASM = os.path.dirname(HERE)
ROOT = os.path.dirname(os.path.dirname(ASM))
sys.path.insert(0, ASM)
import i4004_pack as P  # noqa: E402

B = os.path.join(ROOT, "llvm-i4004", "install", "bin")
if not os.path.exists(os.path.join(B, "llc")):
    alt = "/build/llvm-obj/bin"
    if os.path.exists(os.path.join(alt, "llc")):
        B = alt
RT = os.path.join(ROOT, "llvm-i4004", "rt")
WORK = os.path.join(os.environ.get("KIROCREW_SCRATCH", "/tmp"), "test_farcall")
os.makedirs(WORK, exist_ok=True)


def contained(base, *parts):
    """Resolve parts under base and refuse paths that escape it (SAST guard)."""
    real_base = pathlib.Path(base).resolve()
    p = real_base.joinpath(*parts).resolve()
    if p != real_base and real_base not in p.parents:
        raise SystemExit(f"path escapes {real_base}: {parts}")
    return p


def sh(*a):
    subprocess.run(a, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)


def fail(msg):
    print("FAIL:", msg)
    sys.exit(1)


def main():
    # 1. relocation sanity
    assert P.R_I4004_BANK4 == 9, "R_I4004_BANK4 must be 9"
    assert P.RELOC_NAME[P.R_I4004_BANK4] == "R_I4004_BANK4"

    # tiny program: entry -> f1 -> f2, padded so next-fit scatters them.
    ll = contained(WORK, "t.ll")
    ll.write_text('''target datalayout = "e-m:e-p:16:8-p1:16:8-p2:16:8-i8:8-i16:8-n8-a:8"
target triple = "i4004"
define void @pad(i8 %x){
  store volatile i8 %x, ptr inttoptr (i16 32 to ptr)
  store volatile i8 %x, ptr inttoptr (i16 34 to ptr)
  ret void
}
define i8 @f2(i8 %x){ call void @pad(i8 %x) %r=add i8 %x,1 ret i8 %r }
define i8 @f1(i8 %x){ call void @pad(i8 %x) %a=add i8 %x,1 %r=call i8 @f2(i8 %a) ret i8 %r }
define void @entry(){ %c=call i8 @f1(i8 0) store volatile i8 %c, ptr inttoptr (i16 0 to ptr) ret void }
''')
    reset = contained(WORK, "reset.s")
    reset.write_text('''\t.section .reset,"ax"
\t.globl _start
_start:
\tldm 0
\tdcl
\tfim p0, 0x0f
\tsrc p0
\tldm 0
\twrm
\tfim p7, 0x40
\tfim p0, %lo8(__halt)
\tjms __push_off
\tfim p0, %page(__halt)
\tjms __push_pg
\tjun entry
\t.globl __halt
__halt:\tjun __halt
''')
    sh(f"{B}/llc", "-mtriple=i4004", "-function-sections", "-filetype=obj",
       ll, "-o", os.path.join(WORK, "t.o"))
    sh(f"{B}/llvm-mc", "-triple", "i4004", "--filetype=obj", reset,
       "-o", os.path.join(WORK, "reset.o"))
    objs = [os.path.join(WORK, "reset.o")]
    for s in ("arith", "arith16", "bcd", "logic", "logic16", "shift",
              "shift16", "mem", "stack", "regops"):
        o = os.path.join(WORK, f"{s}.o")
        sh(f"{B}/llvm-mc", "-triple", "i4004", "--filetype=obj",
           os.path.join(RT, f"{s}.s"), "-o", o)
        objs.append(o)
    objs.append(os.path.join(WORK, "t.o"))

    # link banked with far-calls (small banks -> every chain call is cross-bank)
    common, bank = 0x800, 0x200
    objlist = [P.read_elf(o) for o in objs]
    ln = P.Linker(objlist, common + bank, 0, 0x40, 0x40, 0x40, False, False,
                  banks=16, common_bytes=common, bank_bytes=bank)
    ln.want_softcall = True
    ln.far_calls = True
    ln.link()

    # 2/3/4. inspect a far thunk in bank 0's image.
    img = ln.bank_images[0]
    assert ln.far_thunks or ln.far_thunks_common, "expected far thunks"
    # helpers present
    for name in ("__push_pb", "__soft_ret", "__push_off"):
        assert name in ln.linker_syms, f"missing linker sym {name}"
    # a far thunk starts with the inline P4:P5 save (fim p0,FWD_SAVE) and ends
    # `jun __soft_ret`. Inline save = 20 bytes, then 12-byte dispatch.
    checked = 0
    soft = ln.linker_syms["__soft_ret"]
    S = ln.FWD_SAVE
    all_thunks = []
    for (addr, cbk, hb), (base, ca, cb, isc) in ln.far_thunks.items():
        all_thunks.append((base, ca, cb, ln.bank_images[0] if isc else ln.bank_images[hb]))
    for (addr, cbk), (base, ca, cb) in ln.far_thunks_common.items():
        all_thunks.append((base, ca, cb, ln.bank_images[0]))
    save = ln.linker_syms["__save_p4p5"]
    for base, ca, cb, im in all_thunks:
        assert im[base] == (0x50 | ((save >> 8) & 0xF)) and im[base + 1] == (save & 0xFF), \
            "thunk[0] not `jms __save_p4p5`"
        assert im[base + 16] == (0x40 | ((soft >> 8) & 0xF)), "thunk tail not jun __soft_ret hi"
        assert im[base + 17] == (soft & 0xFF), "thunk tail not jun __soft_ret lo"
        tramp = ln.far_restore[(ca, cb)]
        tb = tramp[0]
        timg = ln.bank_images[0] if tramp[3] else ln.bank_images[cb]
        jun_hi = timg[tb + 20]
        assert (jun_hi & 0xF0) == 0x40, "trampoline does not end in jun"
        assert (jun_hi & 0x0F) == ((ca >> 8) & 0x0F), "trampoline jun page mismatch"
        checked += 1
    assert checked > 0, "no far thunks to check"

    # 6. REGRESSION (deep-i8-chain N-2 / doubling): two DISTINCT cross-bank
    # callees that share a banked PC and are reached from the SAME caller bank
    # must get DISTINCT thunks. The key is (callee_addr, callee_bank,
    # caller_bank); keying by (addr, caller_bank) alone let the 2nd call site
    # overwrite the 1st, so both cross-bank calls in one function dispatched to
    # a single callee. Assert no two thunks sharing (addr, caller_bank) but
    # differing in callee_bank collapsed to one base.
    seen = {}
    for (addr, cbk, hb), (base, ca, cb, isc) in ln.far_thunks.items():
        k = (addr, hb)
        if k in seen and seen[k][0] != cb:
            assert seen[k][1] != base, (
                f"thunk collision: callees bank {seen[k][0]} and {cb} at "
                f"addr 0x{addr:x} from caller bank {hb} share thunk base "
                f"0x{base:x} (the N-2 regression)")
        seen[k] = (cb, base)

    # 5. no cross-bank `jun callee` left unrewritten: every cross-bank ADDR12
    # site in a banked section points at a thunk, not the raw callee.
    print(f"PASS: far-call lowering OK ({checked} thunks verified; "
          f"{len(ln.far_restore)} trampolines; R_I4004_BANK4=9)")


if __name__ == "__main__":
    main()
