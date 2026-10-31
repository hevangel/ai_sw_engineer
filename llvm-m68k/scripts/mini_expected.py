#!/usr/bin/env python3
"""Python mirror of rust/smoke/src/mini.rs — hand-verifiable primitive ops.

Ops (slot: semantics, all mod 2^32):
  0  a + b            1  a ^ b            2  a & b
  3  a | b            4  rol(a, 4)        5  a >> 5
  6  (a & 0xFFFF) * (b & 0xFFFF)         7  b - a
  8  bit-reverse of a 9  max(a, b)       10 popcount(a)     11 ror(a, 8)
"""
import struct

M32 = 0xFFFFFFFF
INPUT_BYTES = struct.pack(">II", 0x12345678, 0x9ABCDEF0)
A, B = 0x12345678, 0x9ABCDEF0


def bit_reverse(v):
    r = 0
    for _ in range(32):
        r = (r << 1) | (v & 1)
        v >>= 1
    return r & M32


def window():
    a, b = A, B
    out = [
        (a + b) & M32,
        a ^ b,
        a & b,
        a | b,
        rotl := ((a << 4) | (a >> 28)) & M32,
        (a >> 5) & M32,
        ((a & 0xFFFF) * (b & 0xFFFF)) & M32,
        (b - a) & M32,
        bit_reverse(a),
        max(a, b),
        bin(a).count("1"),
        ((a >> 8) | (a << 24)) & M32,  # ror(a, 8)
    ]
    return [v & M32 for v in out]


if __name__ == "__main__":
    for v in window():
        for sh in (24, 16, 8, 0):
            print("%02x" % ((v >> sh) & 0xFF))
