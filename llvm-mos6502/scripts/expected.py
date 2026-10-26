#!/usr/bin/env python3
"""Independent expected values for the llvm-mos6502 smoke program.

Re-implements rust/smoke/src/main.rs `table_entry` (and the confirmation
cells) in plain Python — the oracle is the SOURCE SEMANTICS, computed without
running the compiled image, the llvm-mos toolchain or any 6502 emulator.
Used by exec_py65.py and exec_mos6502.py; the executors' memory dumps must
match this byte for byte.
"""

WINDOW = 32
RESULT_BASE = 0x0200  # RESULTS address from the linked image (see build.sh output)
DONE_ADDR = 0x0220
DONE_VALUE = 0x5A


def _rotate_right(v, n):
    return ((v >> n) | (v << (8 - n))) & 0xFF


def table_entry(i):
    """Mirror of smoke6502::table_entry with explicit wrapping."""
    wide = (i * 251 + 0x1234) & 0xFFFF          # u16 mul/add, wrapping
    lo = wide & 0xFF
    hi = (wide >> 8) & 0xFF
    rolled = _rotate_right(lo, 3)               # u8 rotate_right(3)
    mixed = ((lo ^ hi) + rolled) & 0xFF         # xor then wrapping_add
    shifted = (((i << 2) & 0xFF) | (i >> 5))    # shl2 | shr5 on u8
    shifted = (shifted - hi) & 0xFF             # wrapping_sub
    if mixed > shifted:
        return (mixed - shifted) & 0xFF
    return (shifted - mixed) & 0xFF


def window():
    """The 32 bytes main() must leave at RESULT_BASE."""
    out = [table_entry(i) for i in range(WINDOW)]
    # confirmation cells written after the loop (indices 30 and 31)
    confirm16 = (0x1234 * 251) & 0xFFFF
    out[WINDOW - 2] = (confirm16 >> 8) & 0xFF
    out[WINDOW - 1] = 0xA5
    return out


if __name__ == "__main__":
    for b in window():
        print("%02x" % b)
