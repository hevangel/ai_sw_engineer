#!/usr/bin/env python3
"""Host oracle for the regops chips tests. Digits are little-endian (digit 0
first); prints the resulting 16 A-digits (probe) space-separated, matching the
rt_test readback order (char 0.. = digit 0..).
"""
import sys


def pad16(csv):
    d = [int(x) for x in csv.split(",")] if csv else []
    return (d + [0] * 16)[:16]


def main():
    op = sys.argv[1]
    if op == "copy":
        a = pad16(sys.argv[2])
        print(" ".join(str(x) for x in a))
    elif op == "clear":
        print(" ".join(["0"] * 16))
    elif op == "add":
        a = pad16(sys.argv[2]); b = pad16(sys.argv[3])
        carry = 0
        for i in range(16):
            s = a[i] + b[i] + carry
            carry = 0
            if s > 9:
                s -= 10; carry = 1
            a[i] = s
        print(" ".join(str(x) for x in a))
    elif op == "sub":
        a = pad16(sys.argv[2]); b = pad16(sys.argv[3])
        borrow = 0
        for i in range(16):
            sub = b[i] + borrow
            if a[i] >= sub:
                a[i] -= sub; borrow = 0
            else:
                a[i] = a[i] + 10 - sub; borrow = 1
        print(" ".join(str(x) for x in a))


if __name__ == "__main__":
    main()
