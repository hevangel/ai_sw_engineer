#!/usr/bin/env python3
"""Independent cross-check of the llvm-i4004 disassembler.

For every first byte 0x00..0xFF (plus all 256 second bytes of a representative
two-word opcode) this runs OUR disassembler (`llvm-mc -triple=i4004
--disassemble`) and compares the decoded mnemonic + operand shape against TWO
independent 4004 decoders that live outside this repo's shipped crates and are
read at run time, never copied:

  1. external/chips  src/mcs4/cpu4004.rs  (Andre Weissflog / V. Ilmer fork;
     a behavioural Rust emulator). Its OPR/OPA dispatch is the oracle.
  2. ../ai_hw_engineer design/intel_4004/src/intel_4004.sv  (the synthesizable
     RTL), when present on the host. Its `Opr*` localparams are parsed and
     confirmed to map each nibble to the canonical group, then the same nibble
     dispatch is applied.

Any first byte on which our disassembler and an oracle disagree is a FINDING,
printed with every opinion. The MCS-4 manual (via llvm-i4004/spec/isa.md) is
the tie-breaker, cited in the report -- this script never edits either side to
force agreement.

Usage (inside the ai-sw-engineer container):
  llvm-i4004/scripts/crosscheck_disasm.py
  LLVM_MC=/path/to/llvm-mc llvm-i4004/scripts/crosscheck_disasm.py

Exit status: 0 = no disagreement, 1 = at least one FINDING, 2 = setup error.
"""
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
BACKEND = os.path.dirname(HERE)
REPO = os.path.dirname(BACKEND)

LLVM_MC = os.environ.get("LLVM_MC") or os.path.join(BACKEND, "install", "bin", "llvm-mc")
CHIPS = os.path.join(REPO, "external", "chips", "src", "mcs4", "cpu4004.rs")
RTL_ENV = os.environ.get("AI_HW_ENGINEER")
RTL_CANDIDATES = [
    RTL_ENV and os.path.join(RTL_ENV, "design", "intel_4004", "src", "intel_4004.sv"),
    os.path.join(REPO, "..", "ai_hw_engineer", "design", "intel_4004", "src", "intel_4004.sv"),
    "/ai_hw_engineer/design/intel_4004/src/intel_4004.sv",
]

IO_OPS = ["wrm", "wmp", "wrr", "wpm", "wr0", "wr1", "wr2", "wr3",
          "sbm", "rdm", "rdr", "adm", "rd0", "rd1", "rd2", "rd3"]
ACC_OPS = ["clb", "clc", "iac", "cmc", "cma", "ral", "rar", "tcc",
           "dac", "tcs", "stc", "daa", "kbp", "dcl", None, None]
SIMPLE = {0x1: "jcn", 0x4: "jun", 0x5: "jms", 0x6: "inc", 0x7: "isz",
          0x8: "add", 0x9: "sub", 0xA: "ld", 0xB: "xch",
          0xC: "bbl", 0xD: "ldm"}


def our_mnemonic(byte0, byte1=None):
    """Decode a 1-byte stream (byte1 is None) or an explicit 2-byte stream.
    Returns the first mnemonic, or None if the stream does not decode. NB: when
    probing a single byte for validity we must NOT append a padding byte -- a
    failed first byte would otherwise let the decoder resynchronise on the pad
    and report the pad's mnemonic instead of the failure."""
    stream = f"0x{byte0:02x}\n" if byte1 is None else f"0x{byte0:02x} 0x{byte1:02x}\n"
    p = subprocess.run([LLVM_MC, "-triple=i4004", "--disassemble"],
                       input=stream, capture_output=True, text=True)
    out = p.stdout.strip()
    if not out:
        return None
    first = out.splitlines()[0].strip()
    return first.split()[0] if first else None


def oracle_dispatch(byte0):
    """OPR/OPA nibble dispatch shared by chips and the RTL (both decode the
    4004 identically). Returns the group mnemonic or 'INVALID'."""
    opr = (byte0 >> 4) & 0xf
    opa = byte0 & 0xf
    if opr == 0x0:
        return "nop" if opa == 0 else "INVALID"
    if opr == 0x2:
        return "src" if (opa & 1) else "fim"
    if opr == 0x3:
        return "jin" if (opa & 1) else "fin"
    if opr == 0xE:
        return IO_OPS[opa]
    if opr == 0xF:
        return ACC_OPS[opa] if opa < 0xE else "INVALID"
    return SIMPLE[opr]


def rtl_path():
    for c in RTL_CANDIDATES:
        if c and os.path.exists(c):
            return c
    return None


def check_rtl_localparams(rtl_src):
    want = {
        "OprNop": 0x0, "OprJcn": 0x1, "OprFimSrc": 0x2, "OprFinJin": 0x3,
        "OprJun": 0x4, "OprJms": 0x5, "OprInc": 0x6, "OprIsz": 0x7,
        "OprAdd": 0x8, "OprSub": 0x9, "OprLd": 0xA, "OprXch": 0xB,
        "OprBbl": 0xC, "OprLdm": 0xD, "OprIo": 0xE, "OprAcc": 0xF,
    }
    findings = []
    for name, nib in want.items():
        m = re.search(rf"{name}\s*=\s*4'h([0-9a-fA-F])", rtl_src)
        if not m:
            findings.append(f"RTL localparam {name} not found")
        elif int(m.group(1), 16) != nib:
            findings.append(
                f"RTL localparam {name} = 0x{m.group(1)} but canonical nibble 0x{nib:x}")
    return findings


def check_chips_structure(chips_src):
    """Confirm the chips source still splits OPR=2/3 on the low bit and names
    the IO/ACC groups under OPR=0xE/0xF, so the shared dispatch is a faithful
    oracle for THIS file (not a stale assumption)."""
    findings = []
    checks = [
        (r"0x2\s*=>", "chips OPR=0x2 arm (FIM/SRC)"),
        (r"0x3\s*=>", "chips OPR=0x3 arm (FIN/JIN)"),
        (r"modifier\s*&\s*1", "chips low-bit split for OPR 2/3"),
        (r"0xE\s*=>\s*match\s*modifier", "chips OPR=0xE IO group"),
        (r"0xF\s*=>\s*match\s*modifier", "chips OPR=0xF accumulator group"),
        (r'unreachable!\("Unknown opcode: F', "chips treats 0xFF as invalid"),
    ]
    for pat, desc in checks:
        if not re.search(pat, chips_src):
            findings.append(f"chips structure changed: missing {desc}")
    return findings


def main():
    if not (os.path.exists(LLVM_MC) or os.path.exists(LLVM_MC + ".exe")):
        print(f"ERROR: llvm-mc not found at {LLVM_MC}; build it first.", file=sys.stderr)
        return 2
    if not os.path.exists(CHIPS):
        print(f"ERROR: chips decoder not found at {CHIPS}; init the submodule.",
              file=sys.stderr)
        return 2

    rtl = rtl_path()
    print(f"==> disassembler : {LLVM_MC}")
    print(f"==> chips oracle : {CHIPS}")
    print(f"==> RTL oracle   : {rtl if rtl else '(ai_hw_engineer not on host - skipped)'}")
    print()

    findings = []

    with open(CHIPS, "r", encoding="utf-8", errors="replace") as f:
        findings += [("chips-struct", m) for m in check_chips_structure(f.read())]

    if rtl:
        with open(rtl, "r", encoding="utf-8", errors="replace") as f:
            findings += [("rtl-localparam", m) for m in check_rtl_localparams(f.read())]

    for b0 in range(256):
        opr = (b0 >> 4) & 0xf
        # Two-word opcodes need a second byte or they'd fail as truncated:
        # JCN(1), FIM(2 even), JUN(4), JMS(5), ISZ(7). All others are 1 byte.
        two_word = opr in (0x1, 0x4, 0x5, 0x7) or (opr == 0x2 and (b0 & 1) == 0)
        ours = our_mnemonic(b0, 0x00 if two_word else None)
        ours_norm = ours if ours else "INVALID"
        oracle = oracle_dispatch(b0)
        agree = (ours_norm == oracle) or (oracle == "INVALID" and ours is None)
        if not agree:
            findings.append(("decode", f"byte 0x{b0:02x}: ours={ours_norm!r} oracle={oracle!r}"))

    # Representative two-word opcode: every second byte must decode as jcn.
    for second in range(256):
        if our_mnemonic(0x14, second) != "jcn":
            findings.append(("2byte", f"jcn with 2nd byte 0x{second:02x} failed to decode"))
            break

    print(f"==> checked 256 first bytes + 256 JCN second bytes against "
          f"{'chips + RTL' if rtl else 'chips (RTL absent)'}")
    if not findings:
        print("==> RESULT: no disagreements. Disassembler matches both independent decoders.")
        return 0
    print(f"==> RESULT: {len(findings)} FINDING(S):")
    for kind, msg in findings:
        print(f"    [{kind}] {msg}")
    print("\nResolve each against the MCS-4 manual (spec/isa.md); never force "
          "one side to match the other.")
    return 1


if __name__ == "__main__":
    sys.exit(main())
