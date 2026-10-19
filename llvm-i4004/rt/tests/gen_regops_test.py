#!/usr/bin/env python3
"""Generate a chips ROM stub exercising one regops primitive on two 16-digit
BCD registers in 4002 RAM, then write the result (register A's 16 digits, or the
P1 return byte) to chip0/reg0 chars for the rt_test harness to read back.

Register A lives at SRC base 0x10 (chip0 reg1, chars 0..15 = 0x10..0x1f).
Register B lives at SRC base 0x20 (chip0 reg2). One BCD digit per char, digit i
at base+i. Args are i16 pointers in D1 (P2) and D2 (P4): we FIM P2=0x10, P4=0x20.

Usage: gen_regops_test.py <sym> --a d0,d1,... --b d0,d1,...   (16 digits each)
Modes that return a byte (reg_add/reg_cmp/reg_is_zero) store it to char0;
modes that modify A (reg_add/reg_sub/reg_copy/reg_clear) have A's digits probed
to chars 0..(nprobe-1). The driver decides what to compare.
"""
import argparse, sys

HEAD = """; GENERATED regops test stub.
	.section .reset,"ax"
reset:
	jun	start
	.text
start:
	ldm	0
	dcl
"""
TAIL = "halt:	jun	halt\n"

A_BASE = 0x10
B_BASE = 0x20


def clear_both():
    # zero the 32 chars 0x10..0x2f (both regions) with a tight loop, 8-bit
    # address advance (char nibble carries into reg nibble at 0x1f->0x20).
    # P0 = address (R0 hi, R1 lo). R2 = down counter (32 -> 0 via DAC).
    return [
        "\tfim\tp0, 0x10",
        "\tldm\t0",
        "\txch\tr2",            # R2 = 0 ; we count 32 up with two passes? use 16x2
        # simplest: 32 iterations via R2 counting down from 0 (wrap) twice is
        # error-prone; instead clear 16 then 16 with INC-with-carry, 32 total.
        "\tldm\t0",
        "\txch\tr3",            # R3 = 0 counter (ISZ wraps after 16)
        ".Lc_a:",
        "\tsrc\tp0",
        "\tldm\t0",
        "\twrm",
        "\tinc\tr1",
        "\tisz\tr3, .Lc_a",     # 16 chars of region A (0x10..0x1f); R1 wraps 0x1f->0x10
        # now advance to 0x20: R1 wrapped to 0, bump R0 to 2
        "\tinc\tr0",            # P0 = 0x20
        "\tldm\t0",
        "\txch\tr3",
        ".Lc_b:",
        "\tsrc\tp0",
        "\tldm\t0",
        "\twrm",
        "\tinc\tr1",
        "\tisz\tr3, .Lc_b",     # 16 chars of region B (0x20..0x2f)
    ]


def seed(base, digits):
    # write only the nonzero digits (regions pre-cleared).
    s = []
    for i, d in enumerate(digits):
        if d & 0xf == 0:
            continue
        s.append(f"\tfim\tp0, 0x{(base + i) & 0xff:02x}")
        s.append("\tsrc\tp0")
        s.append(f"\tldm\t{d & 0xf}")
        s.append("\twrm")
    return s


def probe_A(nprobe):
    # copy A[0..nprobe) from 0x10.. to reg0 chars 0..nprobe-1
    s = []
    for i in range(nprobe):
        s.append(f"\tfim\tp0, 0x{(A_BASE + i) & 0xff:02x}")
        s.append("\tsrc\tp0")
        s.append("\trdm")
        s.append("\txch\tr6")
        s.append(f"\tfim\tp0, 0x{i:02x}")
        s.append("\tsrc\tp0")
        s.append("\tld\tr6")
        s.append("\twrm")
    return s


def store_ret():
    # P1 return (R3 low nibble) -> reg0 char0
    return ["\tfim\tp0, 0x00", "\tsrc\tp0", "\tld\tr3", "\twrm"]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("sym")
    ap.add_argument("--a", default="")
    ap.add_argument("--b", default="")
    ap.add_argument("--nprobe", type=int, default=16)
    ap.add_argument("--ret", action="store_true", help="store P1 return to char0")
    args = ap.parse_args()
    A = [int(x) for x in args.a.split(",")] if args.a else []
    B = [int(x) for x in args.b.split(",")] if args.b else []
    A = (A + [0] * 16)[:16]
    B = (B + [0] * 16)[:16]
    body = []
    body += clear_both()
    body += seed(A_BASE, A)
    body += seed(B_BASE, B)
    body.append("\tfim\tp2, 0x10")  # D1 low byte = a addr
    body.append("\tfim\tp3, 0x00")
    body.append("\tfim\tp4, 0x20")  # D2 low byte = b addr
    body.append("\tfim\tp5, 0x00")
    body.append(f"\tjms\t{args.sym}")
    if args.ret:
        body += store_ret()
    else:
        body += probe_A(args.nprobe)
    sys.stdout.write(HEAD + "\n".join(body) + "\n" + TAIL)


if __name__ == "__main__":
    main()
