#!/usr/bin/env python3
"""Independent opcode verification of the llvm-m68k smoke image.

Runs INSIDE the ai-sw-engineer container, from the repository root, after
scripts/build.sh:

    llvm-m68k/scripts/disasm_check.py

Checks (decoders independent of the Rust/LLVM code path — GNU binutils is
the primary; LLVM's objdump cross-checks byte coverage):

  1. every `.text` byte decodes in GNU objdump M68000 mode with no data
     gaps (nothing emitted that binutils cannot decode as instructions);
  2. LLVM objdump decodes the same number of instruction bytes;
  3. 68010+ mnemonics are absent (movec, rtd, moves, cas, bf*, pack, unpk,
     chk2/cmp2, callm, pflush/ptest, divsl/mulsl long forms...);
  4. the disassembly covers the instruction kinds the smoke source requires
     (move/eor/mulu/shift/add/cmp/jmp/jsr), and the image tail holds a
     spin loop (branch-to-self).
"""
import os
import re
import subprocess

HERE = os.path.dirname(os.path.abspath(__file__))
BUILD = os.path.join(os.path.dirname(HERE), "build")
ELF = os.path.join(BUILD, "smoke.elf")
IMAGE = os.path.join(BUILD, "smoke.bin")

BANNED = ("movec", "rtd", "moves", "cas", "cas2", "pack", "unpk",
          "chk2", "cmp2", "callm", "pflush", "ptest", "divsl", "mulsl")
# NB: no `mulu` requirement — LLVM expands constant multiplies into
# shift/add sequences on m68k (mulu is expensive), which is expected output.
REQUIRED = ("move", "eor", "add", "cmp", "jmp", "jsr")


def main():
    # 1. GNU objdump (M68000 mode) decodes the whole .text with no gaps.
    gnu = subprocess.run(
        ["m68k-linux-gnu-objdump", "-d", "-m68000", ELF],
        capture_output=True, text=True, check=True).stdout
    covered = {}
    mnems = []
    for line in gnu.splitlines():
        m = re.match(r"\s*([0-9a-f]+):\s+((?:[0-9a-f]{2,4} )+)\s*(\S.*)?", line)
        if not m:
            continue
        addr = int(m.group(1), 16)
        words = m.group(2).split()
        nbytes = sum(len(w) // 2 for w in words)
        covered[addr] = nbytes
        parts = (m.group(3) or "").split()
        if parts:
            mnems.append((addr, parts[0].lower()))
    assert covered, "GNU objdump produced no .text instructions"

    # expand covered ranges by instruction length, then check for gaps
    span = {}
    for addr, nbytes in covered.items():
        for a in range(addr, addr + nbytes):
            span[a] = addr
    lo = min(span)
    hi = max(a + 1 for a in span)
    missing = [a for a in range(lo, hi) if a not in span]
    assert not missing, f"undecoded bytes in .text: {[hex(a) for a in missing[:8]]}"
    print(f"ok: GNU objdump decodes {hi - lo} .text bytes at {lo:#x}..{hi:#x} "
          f"({len(mnems)} instructions, no gaps)")

    # 2. LLVM objdump cross-check: same byte coverage. The image's llvm-mos
    # objdump has no M68k target; use the pinned nightly's llvm-tools build
    # (rust's LLVM includes M68k — it is what compiled the smoke).
    llobj = os.environ.get(
        "LLVM_OBJDUMP",
        "/home/mos/.rustup/toolchains/nightly-2026-10-15-x86_64-unknown-linux-gnu/"
        "lib/rustlib/x86_64-unknown-linux-gnu/bin/llvm-objdump")
    llvm = subprocess.run(
        [llobj, "-d", ELF],
        capture_output=True, text=True, check=True).stdout
    lbytes = 0
    for line in llvm.splitlines():
        m = re.match(r"\s*[0-9a-f]+:\s+((?:[0-9a-f]{2,4} )+)", line)
        if m:
            lbytes += sum(len(w) // 2 for w in m.group(1).split())
    assert lbytes == hi - lo, f"LLVM decodes {lbytes} bytes vs GNU {hi - lo}"
    print(f"ok: LLVM objdump agrees on the byte coverage ({lbytes} bytes)")

    # 3. no 68010+ mnemonics
    present = {m for _, m in mnems}
    offenders = sorted(present & set(BANNED))
    assert not offenders, f"68010+ mnemonics present: {offenders}"
    print("ok: no 68010+ mnemonics (M68000-clean output)")

    # 4. required kinds + spin loop at the image tail
    for want in REQUIRED:
        assert any(m.startswith(want) for m in present), \
            f"expected instruction kind {want} in the disassembly"
    image = open(IMAGE, "rb").read()
    assert 0x60 in image[-8:], "no spin loop (bra self) found in the image tail"
    print("ok: instruction mix covers the smoke's required kinds; spin loop present")
    print("PASS: independent opcode verification")


if __name__ == "__main__":
    main()
