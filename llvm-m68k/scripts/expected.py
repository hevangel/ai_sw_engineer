#!/usr/bin/env python3
"""Independent expected values for the llvm-m68k smoke program.

Re-implements rust/smoke/src/main.rs `table_entry` (and the confirmation
cells) in plain Python with explicit 32-bit masks — the oracle is the source
semantics, computed without running the compiled image, rustc/LLVM or any
emulator. Used by exec_r68k.sh; the executor's memory dump must match this
byte for byte.
"""

WINDOW = 32
INPUT_ADDR = 0x1000
OUTPUT_ADDR = 0x1100
DONE_ADDR = 0x1200
DONE_VALUE = 0x5A

# The stimulus the executor writes to the INPUT window (mirrored here).
INPUT_BYTES = [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0]

M32 = 0xFFFFFFFF


def rotl(v, n):
    n %= 32
    return ((v << n) | (v >> (32 - n))) & M32


def table_entry(i, a, b):
    """Mirror of smokem68k::table_entry with explicit wrapping."""
    wide = ((i * 0x0101_0101) & M32) ^ rotl(a, (i % 7) + 1)
    mixed = ((wide * 251) & M32) + (b & 0x00FF_00FF)
    mixed &= M32
    shifted = ((mixed >> 3) | ((mixed << 29) & M32)) & M32  # rotate via >3 | <<29
    counted = bin(shifted).count("1")
    ref = shifted & 0x3F
    picked = counted if counted > ref else ref
    return (picked ^ (mixed >> 24)) & 0xFF


def window():
    a = (INPUT_BYTES[0] << 24) | (INPUT_BYTES[1] << 16) | (INPUT_BYTES[2] << 8) | INPUT_BYTES[3]
    b = (INPUT_BYTES[4] << 24) | (INPUT_BYTES[5] << 16) | (INPUT_BYTES[6] << 8) | INPUT_BYTES[7]
    out = [table_entry(i, a, b) for i in range(WINDOW)]
    confirm = (a * b) & M32
    out[WINDOW - 2] = (confirm >> 24) & 0xFF
    out[WINDOW - 1] = 0xA5
    return out


if __name__ == "__main__":
    for byte in window():
        print("%02x" % byte)
