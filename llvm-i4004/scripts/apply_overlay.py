#!/usr/bin/env python3
"""Apply the I4004 overlay onto a scratch copy of the LLVM source tree.

This never touches external/llvm-project (the pinned submodule). It is given
the path to a *copy* / worktree of the LLVM tree ($LLVM_SRC) and:

  1. copies llvm-i4004/target/lib/Target/I4004 -> $LLVM_SRC/llvm/lib/Target/I4004
  2. installs the ELF relocs .def into $LLVM_SRC/llvm/include/...
  3. makes small, idempotent, anchored edits to the core infra files:
       - Triple.h / Triple.cpp  (register the i4004 arch)
       - ELF.h                  (EM_I4004 + R_I4004_* relocation enum)
       - ELF.cpp / ELFObjectFile.h (nice-to-have machine name + getArch)

All edits are idempotent: running twice is a no-op. Anchored on unique strings
so they survive minor upstream whitespace changes.
"""
import os
import pathlib
import shutil
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
OVERLAY = os.path.normpath(os.path.join(HERE, "..", "target"))

# Root of the LLVM tree this run may write into; set by main().
_TREE = None


def write(p, s):
    p = pathlib.Path(p).resolve()
    if pathlib.Path(_TREE).resolve() not in p.parents:
        raise SystemExit(f"refusing write outside the LLVM tree: {p}")
    p.write_text(s, encoding="utf-8", newline="\n")


def read(p):
    with open(p, "r", encoding="utf-8") as f:
        return f.read()


def ensure(path, anchor, insert, label, after=True, once_marker=None):
    """Insert `insert` before/after the line containing `anchor` if the
    marker (default: insert itself) is not already present."""
    s = read(path)
    marker = once_marker if once_marker is not None else insert.strip().splitlines()[0]
    if marker in s:
        print(f"  = {label}: already present")
        return
    idx = s.find(anchor)
    if idx < 0:
        print(f"  ! {label}: ANCHOR NOT FOUND: {anchor!r}", file=sys.stderr)
        sys.exit(3)
    # Expand to the end (after) or start (before) of the anchor's line.
    line_start = s.rfind("\n", 0, idx) + 1
    line_end = s.find("\n", idx)
    if line_end < 0:
        line_end = len(s)
    if after:
        s = s[: line_end + 1] + insert + s[line_end + 1 :]
    else:
        s = s[:line_start] + insert + s[line_start:]
    write(path, s)
    print(f"  + {label}: inserted")


def main():
    if len(sys.argv) != 2:
        print("usage: apply_overlay.py <LLVM_SRC>", file=sys.stderr)
        sys.exit(2)
    src = os.path.abspath(sys.argv[1])
    global _TREE
    _TREE = src
    llvm = os.path.join(src, "llvm")
    if not os.path.isdir(llvm):
        print(f"not an LLVM tree: {src}", file=sys.stderr)
        sys.exit(2)

    # 1. Target sources.
    tgt_src = os.path.join(OVERLAY, "lib", "Target", "I4004")
    tgt_dst = os.path.join(llvm, "lib", "Target", "I4004")
    print(f"==> copy target sources -> {tgt_dst}")
    if os.path.isdir(tgt_dst):
        shutil.rmtree(tgt_dst)
    shutil.copytree(tgt_src, tgt_dst)

    # 2. ELF relocs .def.
    reloc_src = os.path.join(
        OVERLAY, "llvm-include", "llvm", "BinaryFormat", "ELFRelocs", "I4004.def"
    )
    reloc_dst = os.path.join(
        llvm, "include", "llvm", "BinaryFormat", "ELFRelocs", "I4004.def"
    )
    print("==> install ELFRelocs/I4004.def")
    shutil.copyfile(reloc_src, reloc_dst)

    # 3a. Triple.h: arch enum.
    th = os.path.join(llvm, "include", "llvm", "TargetParser", "Triple.h")
    ensure(
        th,
        "msp430,      // MSP430: msp430",
        "    i4004,       // Intel 4004 (MCS-4) [experimental]\n",
        "Triple.h ArchType",
        after=True,
        once_marker="i4004,       // Intel 4004",
    )

    # 3b. Triple.cpp: getArchTypeName, prefix, parseArch, format.
    tc = os.path.join(llvm, "lib", "TargetParser", "Triple.cpp")
    ensure(
        tc,
        'return "msp430";',
        '  case i4004:\n    return "i4004";\n',
        "Triple.cpp getArchTypeName",
        after=True,
        once_marker='return "i4004";',
    )
    ensure(
        tc,
        '.Case("msp430", msp430)',
        '      .Case("i4004", i4004)\n',
        "Triple.cpp getArchTypePrefix/StringSwitch",
        after=True,
        once_marker='.Case("i4004", i4004)',
    )
    ensure(
        tc,
        '.Case("msp430", Triple::msp430)',
        '          .Case("i4004", Triple::i4004)\n',
        "Triple.cpp parseArch",
        after=True,
        once_marker='.Case("i4004", Triple::i4004)',
    )
    ensure(
        tc,
        "case Triple::msp430:\n  case Triple::nvptx64:",
        "  case Triple::i4004:\n",
        "Triple.cpp getDefaultFormat (ELF)",
        after=False,  # insert before the msp430 line of this block
        once_marker="case Triple::i4004:",
    )
    ensure(
        tc,
        "case llvm::Triple::msp430:\n    return 16;",
        "  case llvm::Triple::i4004:\n",
        "Triple.cpp getArchPointerBitWidth (16-bit)",
        after=False,  # fall through to msp430's "return 16;"
        once_marker="case llvm::Triple::i4004:",
    )

    # 3c. ELF.h: EM_I4004 + relocs enum.
    eh = os.path.join(llvm, "include", "llvm", "BinaryFormat", "ELF.h")
    ensure(
        eh,
        "EM_LOONGARCH = 258,     // LoongArch",
        "  EM_I4004 = 0x4004,      // Intel 4004 (MCS-4) [private/experimental]\n",
        "ELF.h EM_I4004",
        after=True,
        once_marker="EM_I4004 = 0x4004",
    )
    ensure(
        eh,
        "// ELF Relocation type for VE.",
        '// ELF Relocation types for Intel 4004\nenum {\n#include "ELFRelocs/I4004.def"\n};\n\n',
        "ELF.h R_I4004 relocs enum",
        after=False,
        once_marker='#include "ELFRelocs/I4004.def"',
    )

    # 3d. ELF.cpp machine-name + reloc-name dispatch (nice-to-have).
    ecpp = os.path.join(llvm, "lib", "Object", "ELF.cpp")
    if os.path.exists(ecpp):
        ensure(
            ecpp,
            "case ELF::EM_MSP430:\n    switch (Type) {",
            "  case ELF::EM_I4004:\n    switch (Type) {\n#include \"llvm/BinaryFormat/ELFRelocs/I4004.def\"\n    default:\n      break;\n    }\n    break;",
            "ELF.cpp getRelocationTypeName",
            after=False,
            once_marker="case ELF::EM_I4004:",
        )

    # 3e. ELFObjectFile.h: file-format name + getArch (so llvm-readobj names it).
    eof = os.path.join(llvm, "include", "llvm", "Object", "ELFObjectFile.h")
    if os.path.exists(eof):
        ensure(
            eof,
            'return "elf32-msp430";',
            '    case ELF::EM_I4004:\n      return "elf32-i4004";\n',
            "ELFObjectFile.h getFileFormatName",
            after=True,
            once_marker='return "elf32-i4004";',
        )
        ensure(
            eof,
            "return Triple::msp430;",
            "  case ELF::EM_I4004:\n    return Triple::i4004;\n",
            "ELFObjectFile.h getArch",
            after=True,
            once_marker="return Triple::i4004;",
        )

    # 3f. llvm-readobj ELFDumper: give the private machine a readable name.
    dumper = os.path.join(llvm, "tools", "llvm-readobj", "ELFDumper.cpp")
    if os.path.exists(dumper):
        ensure(
            dumper,
            'ENUM_ENT(EM_LOONGARCH,     "LoongArch"),',
            '    ENUM_ENT(EM_I4004,         "Intel 4004 (MCS-4)"),\n',
            "ELFDumper EM_I4004 name",
            after=True,
            once_marker="EM_I4004,         \"Intel 4004",
        )

    print("==> overlay applied OK")


if __name__ == "__main__":
    main()
